import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        hive: {
          yellow: "#F5A623",
          dark:   "#0D0D0D",
          card:   "#1A1A1A",
          border: "#2A2A2A",
        },
      },
      fontFamily: {
        mono: ["var(--font-mono)", "monospace"],
      },
    },
  },
  plugins: [],
};

export default config;
