/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        mono: ['"JetBrains Mono"', '"Fira Code"', 'ui-monospace', 'monospace'],
        sans: ['"Inter"', 'ui-sans-serif', 'system-ui', 'sans-serif'],
      },
      colors: {
        brand: {
          blue:   "#3b9eff",
          purple: "#9f7aea",
          green:  "#22d3a5",
          amber:  "#f6ad3c",
          red:    "#fc6675",
        },
      },
      borderRadius: {
        xl2: "20px",
      },
      backgroundImage: {
        "gradient-brand": "linear-gradient(135deg, #60b3ff, #a78bfa, #34d3a5)",
      },
    },
  },
  plugins: [],
};
