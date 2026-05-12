/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  webpack: (config, { isServer }) => {
    if (isServer) {
      // sodium-native is a Node native addon — exclude from webpack bundle
      config.externals = [...(config.externals ?? []), "sodium-native"];
    }
    return config;
  },
};

module.exports = nextConfig;
