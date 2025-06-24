#!/bin/bash

ICON_SIZES=("16" "32" "64" "128" "256" "512" "1024")

mkdir -p ./icon.iconset

for size in "${ICON_SIZES[@]}"; do
    sips -z $size $size -s format png "./${size}.png" --out "./icon.iconset/icon_${size}x${size}.png"
done

iconutil -c icns "./icon.iconset"

rm -Rf ./icon.iconset
