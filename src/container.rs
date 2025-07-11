use std::{collections::HashMap, path::PathBuf};

use bollard::{
    Docker,
    models::*,
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
        ListNetworksOptionsBuilder, RemoveContainerOptionsBuilder, StartContainerOptionsBuilder,
        StopContainerOptionsBuilder,
    },
    secret::{ContainerCreateBody, NetworkCreateRequest},
};
use futures_util::StreamExt;

use crate::{
    config::{LlmChatConfig, LlmChatConfigExtraBackendService},
    error::AppError,
};

/// The Open WebUI container image name and tag.
const OPEN_WEBUI_IMAGE_BASE: &'static str = "ghcr.io/open-webui/open-webui";

/// The Apache Tika container image name and tag.
const TIKA_IMAGE_BASE: &'static str = "docker.io/apache/tika";

/// Pulls the required container images.
///
/// # Arguments
///
/// * `app_config` - The application configuration.
pub async fn pull_required_images(app_config: &LlmChatConfig) -> Result<(), AppError> {
    let open_webui_image = format!(
        "{}:{}",
        OPEN_WEBUI_IMAGE_BASE, app_config.openwebui_image_tag
    );
    let tika_image = format!("{}:{}", TIKA_IMAGE_BASE, app_config.tika_image_tag);

    let mut images = vec![open_webui_image, tika_image];

    if let Some(extra_services) = app_config.extra_backend_services.clone() {
        for extra_service in extra_services {
            images.push(extra_service.image);
        }
    }

    for image in images {
        pull_image(&image).await?;
    }

    Ok(())
}

/// Pull a container image.
///
/// # Arguments
///
/// * `image` - The image to pull.
///
/// # Examples
///
/// ```,no_run
/// let image = "ubuntu:24.04";
///
/// pull_image(image).await;
/// ```
async fn pull_image(image: &str) -> Result<(), AppError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let create_image_opts = CreateImageOptionsBuilder::new().from_image(image).build();

    let mut pull_stream = docker.create_image(Some(create_image_opts), None, None);

    while let Some(msg) = pull_stream.next().await {
        match msg {
            Ok(msg) => println!("{:?}: {:?}", msg.id, msg.status),

            Err(err) => return Err(AppError::DockerError(err)),
        }
    }

    Ok(())
}

/// Creates the infrastructure needed to run the application.
///
/// This includes:
///
/// * The frontend and backend networks.
/// * Running Open WebUI.
/// * Running any backend services needed for Open WebUI.
///   * For example, Apache Tika.
pub async fn create_infrastructure(
    app_config: &LlmChatConfig,
    data_dir: &PathBuf,
) -> Result<(), AppError> {
    let _ = create_frontend_network().await?;
    let _ = create_backend_network().await?;

    create_openwebui_container(app_config, data_dir).await?;
    create_tika_container(app_config).await?;

    if let Some(extra_services) = app_config.extra_backend_services.clone() {
        for extra_service in extra_services {
            create_extra_service_container(&extra_service).await?;
        }
    }

    Ok(())
}

/// Creates the `local_llm_frontend` network with Docker (or any Docker-compatible API).
async fn create_frontend_network() -> Result<NetworkCreateResponse, AppError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let mut driver_opts = HashMap::<String, String>::new();
    driver_opts.insert(
        "com.docker.network.bridge.host_binding_ipv4".to_string(),
        "127.0.0.1".to_string(),
    );

    let network = docker
        .create_network(NetworkCreateRequest {
            name: "local_llm_frontend".to_string(),
            driver: Some("bridge".to_string()),
            options: Some(driver_opts),
            ..Default::default()
        })
        .await
        .map_err(|e| AppError::DockerError(e))?;

    Ok(network)
}

/// Creates the `local_llm_backend` network with Docker (or any Docker-compatible API).
async fn create_backend_network() -> Result<NetworkCreateResponse, AppError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let network = docker
        .create_network(NetworkCreateRequest {
            name: "local_llm_backend".to_string(),
            ..Default::default()
        })
        .await
        .map_err(|e| AppError::DockerError(e))?;

    Ok(network)
}

/// Creates and starts the Open WebUI container with Docker (or any Docker-compatible API).
///
/// # Arguments
///
/// * `app_config` - The application configuration.
/// * `data_dir` - The host path to data directory to mount into the container.
///
/// # Notes
///
/// The name of the container will always be `local_llm_openwebui`.
async fn create_openwebui_container(
    app_config: &LlmChatConfig,
    data_dir: &PathBuf,
) -> Result<(), AppError> {
    let open_webui_image = format!(
        "{}:{}",
        OPEN_WEBUI_IMAGE_BASE, app_config.openwebui_image_tag
    );
    let data_dir = data_dir.to_string_lossy().to_string();

    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let create_container_opts = CreateContainerOptionsBuilder::new()
        .name("local_llm_openwebui")
        .build();

    let container_env = vec![
        "ENV=dev".to_string(),
        "WEBUI_AUTH=false".to_string(),
        //"WEB_LOADER_ENGINE=playwright".to_string(),
        //"PLAYWRIGHT_WS_URI=ws://playwright:3000".to_string(),
    ];

    let mut networks = HashMap::<String, EndpointSettings>::new();
    networks.insert(
        "local_llm_frontend".to_string(),
        EndpointSettings::default(),
    );
    networks.insert("local_llm_backend".to_string(), EndpointSettings::default());

    let networking_config = NetworkingConfig {
        endpoints_config: Some(networks),
    };

    let mut container_ports = HashMap::<String, HashMap<(), ()>>::new();
    container_ports.insert("8080/tcp".to_string(), HashMap::default());

    let mut port_binds = HashMap::<String, Option<Vec<PortBinding>>>::new();
    port_binds.insert(
        "8080/tcp".to_string(),
        Some(vec![PortBinding {
            host_port: Some("11690".to_string()),
            ..Default::default()
        }]),
    );

    let host_config = HostConfig {
        binds: Some(vec![format!("{}:/app/backend/data", data_dir)]),
        port_bindings: Some(port_binds),
        ..Default::default()
    };

    let container_config = ContainerCreateBody {
        image: Some(open_webui_image),
        env: Some(container_env),
        networking_config: Some(networking_config),
        exposed_ports: Some(container_ports),
        host_config: Some(host_config),
        ..Default::default()
    };

    docker
        .create_container(Some(create_container_opts), container_config)
        .await
        .map_err(|e| AppError::DockerError(e))?;

    let start_container_opts = StartContainerOptionsBuilder::new().build();

    docker
        .start_container("local_llm_openwebui", Some(start_container_opts))
        .await
        .map_err(|e| AppError::DockerError(e))?;

    Ok(())
}

/// Creates and starts the Apache Tika container with Docker (or any Docker-compatible API).
///
/// # Arguments
///
/// * `app_config` - The application configuration.
///
/// # Notes
///
/// The name of the container will always be `local_llm_tika`.
async fn create_tika_container(app_config: &LlmChatConfig) -> Result<(), AppError> {
    let tika_image = format!("{}:{}", TIKA_IMAGE_BASE, app_config.tika_image_tag);

    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let create_container_opts = CreateContainerOptionsBuilder::new()
        .name("local_llm_tika")
        .build();

    let mut networks = HashMap::<String, EndpointSettings>::new();
    networks.insert("local_llm_backend".to_string(), EndpointSettings::default());

    let networking_config = NetworkingConfig {
        endpoints_config: Some(networks),
    };

    let mut container_ports = HashMap::<String, HashMap<(), ()>>::new();
    container_ports.insert("9998/tcp".to_string(), HashMap::default());

    let container_config = ContainerCreateBody {
        image: Some(tika_image),
        networking_config: Some(networking_config),
        exposed_ports: Some(container_ports),
        ..Default::default()
    };

    docker
        .create_container(Some(create_container_opts), container_config)
        .await
        .map_err(|e| AppError::DockerError(e))?;

    let start_container_opts = StartContainerOptionsBuilder::new().build();

    docker
        .start_container("local_llm_tika", Some(start_container_opts))
        .await
        .map_err(|e| AppError::DockerError(e))?;

    Ok(())
}

/// Creates and starts an extra backend container with Docker (or any Docker-compatible API).
///
/// # Arguments
///
/// * `extra_service` - The extra service config.
///
/// # Notes
///
/// The name of the container will always be `local_llm_{name}`.
async fn create_extra_service_container(
    extra_service: &LlmChatConfigExtraBackendService,
) -> Result<(), AppError> {
    let container_name = format!("local_llm_{}", extra_service.name);

    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let create_container_opts = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    let container_env = extra_service.env.clone();

    let mut networks = HashMap::<String, EndpointSettings>::new();
    networks.insert("local_llm_backend".to_string(), EndpointSettings::default());

    let networking_config = NetworkingConfig {
        endpoints_config: Some(networks),
    };

    let mut container_ports = HashMap::<String, HashMap<(), ()>>::new();
    if let Some(ports) = extra_service.ports.clone() {
        for port in ports {
            container_ports.insert(port, HashMap::default());
        }
    }

    let host_config = match &extra_service.volume_bindings {
        Some(volume_bindings) => {
            let mut host_binds: Vec<String> = vec![];

            for volume in volume_bindings {
                host_binds.push(format!(
                    "{host_path}:{container_path}",
                    host_path = volume.host_path,
                    container_path = volume.container_path
                ));
            }

            Some(HostConfig {
                binds: Some(host_binds),
                ..Default::default()
            })
        }

        None => None,
    };

    let container_config = ContainerCreateBody {
        image: Some(extra_service.image.clone()),
        cmd: extra_service.cmd.clone(),
        env: container_env,
        networking_config: Some(networking_config),
        exposed_ports: Some(container_ports),
        host_config: host_config,
        user: extra_service.user.clone(),
        working_dir: extra_service.working_directory.clone(),
        ..Default::default()
    };

    docker
        .create_container(Some(create_container_opts), container_config)
        .await
        .map_err(|e| AppError::DockerError(e))?;

    let start_container_opts = StartContainerOptionsBuilder::new().build();

    docker
        .start_container(&container_name, Some(start_container_opts))
        .await
        .map_err(|e| AppError::DockerError(e))?;

    Ok(())
}

/// Cleans up Docker (or any Docker-compatible API) resources created by the application.
///
/// # Arguments
///
/// * `app_config` - The application configuration.
pub async fn cleanup_infrastructure(app_config: &LlmChatConfig) -> Result<(), AppError> {
    println!("Deleting containers...");
    delete_containers(app_config).await?;

    println!("Deleting networks...");
    delete_networks().await?;

    Ok(())
}

/// Delete the `local_llm_frontend` and `local_llm_backend` networks from Docker (or any Docker-compatible API).
async fn delete_networks() -> Result<(), AppError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let mut network_filters = HashMap::<String, Vec<String>>::new();
    network_filters.insert("name".to_string(), vec!["local_llm_".to_string()]);

    let list_network_opts = ListNetworksOptionsBuilder::new()
        .filters(&network_filters)
        .build();

    let container_networks = docker
        .list_networks(Some(list_network_opts))
        .await
        .map_err(|e| AppError::DockerError(e))?;

    for network in container_networks {
        let network_name = network.name.unwrap();

        docker
            .remove_network(&network_name)
            .await
            .map_err(|e| AppError::DockerError(e))?;

        println!("Removed network '{}'", &network_name);
    }

    Ok(())
}

/// Delete the containers created by the application from Docker (or any Docker-compatible API).
///
/// # Arguments
///
/// * `app_config` - The application configuration.
async fn delete_containers(app_config: &LlmChatConfig) -> Result<(), AppError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| AppError::DockerError(e))?;

    let mut container_names = vec![
        "local_llm_openwebui".to_string(),
        "local_llm_tika".to_string(),
    ];

    if let Some(extra_services) = app_config.extra_backend_services.clone() {
        for extra_service in extra_services {
            container_names.push(format!("local_llm_{}", extra_service.name));
        }
    }

    let mut container_filters = HashMap::<String, Vec<String>>::new();
    container_filters.insert("name".to_string(), container_names);

    println!("Getting containers");
    let list_containers_opts = ListContainersOptionsBuilder::new()
        .all(true)
        .filters(&container_filters)
        .build();

    let containers = docker
        .list_containers(Some(list_containers_opts))
        .await
        .map_err(|e| AppError::DockerError(e))?;

    for container in containers {
        let container_names = container.names.unwrap();
        let container_name = container_names.first().unwrap().trim_matches('/');

        let stop_container_opts = StopContainerOptionsBuilder::new().build();

        let _ = docker
            .stop_container(&container_name, Some(stop_container_opts))
            .await;

        let remove_container_opts = RemoveContainerOptionsBuilder::new().force(true).build();

        docker
            .remove_container(&container_name, Some(remove_container_opts))
            .await
            .map_err(|e| AppError::DockerError(e))?;

        println!("Removed container '{}'", &container_name);
    }

    Ok(())
}
