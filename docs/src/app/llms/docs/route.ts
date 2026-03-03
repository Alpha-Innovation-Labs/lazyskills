import { source } from '@/lib/source';

const ghPagesEnabled = process.env.DOCS_GH_PAGES === '1';
export const dynamic = 'force-static';
export const revalidate = false;

export async function GET(req: Request) {
  if (ghPagesEnabled) {
    return new Response('Markdown export is disabled in GitHub Pages mode.', {
      status: 404,
      headers: {
        'content-type': 'text/plain; charset=utf-8',
      },
    });
  }

  const url = new URL(req.url);
  const pathParam = url.searchParams.get('path') ?? '';
  const slug = pathParam.split('/').filter(Boolean);
  const page = source.getPage(slug);
  if (!page) {
    return new Response('Page not found', {
      status: 404,
      headers: {
        'content-type': 'text/plain; charset=utf-8',
      },
    });
  }

  const markdown = await page.data.getText('processed');
  return new Response(markdown, {
    headers: {
      'content-type': 'text/markdown; charset=utf-8',
    },
  });
}
