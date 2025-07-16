/** @type {import('tailwindcss').Config} */
module.exports = {
  content: {
    files: ["./public/*.html"],
    transform: {
      rs: (content) => content.replace(/(?:^|\s)class:/g, " "),
    },
  },
  theme: {
    container: {
      padding: {
        DEFAULT: "1rem",
        sm: "1rem",
        lg: "3rem",
        xl: "7rem",
        "2xl": "8rem",
      },
    },
    extend: {},
  },
  plugins: [],
};
