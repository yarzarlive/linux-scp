/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        commander: {
          dark: "rgb(var(--commander-dark) / <alpha-value>)",
          darker: "rgb(var(--commander-darker) / <alpha-value>)",
          accent: "rgb(var(--commander-accent) / <alpha-value>)",
          border: "rgb(var(--commander-border) / <alpha-value>)",
          surface: "rgb(var(--commander-surface) / <alpha-value>)",
          selected: "rgb(var(--commander-selected) / <alpha-value>)",
          text: "rgb(var(--commander-text) / <alpha-value>)",
          subtext: "rgb(var(--commander-subtext) / <alpha-value>)",
        },
      },
    },
  },
  plugins: [],
}

