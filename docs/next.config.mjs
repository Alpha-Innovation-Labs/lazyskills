import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();
const ghPagesEnabled = process.env.DOCS_GH_PAGES === '1';
const repoName = process.env.DOCS_REPO_NAME;
const explicitBasePath = process.env.DOCS_BASE_PATH;
const docsBasePath =
  ghPagesEnabled
    ? explicitBasePath !== undefined
      ? explicitBasePath || undefined
      : repoName
        ? `/${repoName}`
        : undefined
    : undefined;

/** @type {import('next').NextConfig} */
const config = {
  serverExternalPackages: ['@takumi-rs/image-response'],
  reactStrictMode: true,
  output: ghPagesEnabled ? 'export' : undefined,
  trailingSlash: ghPagesEnabled,
  basePath: docsBasePath,
  assetPrefix: docsBasePath,
  ...(ghPagesEnabled
    ? {}
    : {
        async rewrites() {
          return [
            {
              source: '/docs/:path*.mdx',
              destination: '/llms/docs?path=:path*',
            },
          ];
        },
      }),
};

export default withMDX(config);
