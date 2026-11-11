/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        commander: {
          dark: "#1e1e2e",
          darker: "#181825",
          accent: "#89b4fa",
          border: "#313244",
          surface: "#11111b",
          selected: "#45475a",
          text: "#cdd6f4",
          subtext: "#a6adc8",
        },
      },
    },
  },
  plugins: [],
}
