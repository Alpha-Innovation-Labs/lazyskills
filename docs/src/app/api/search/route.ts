import { source } from '@/lib/source';
import { createFromSource } from 'fumadocs-core/search/server';

const ghPagesEnabled = process.env.DOCS_GH_PAGES === '1';
export const dynamic = 'force-static';
export const revalidate = false;

const searchHandler = createFromSource(source, {
  language: 'english',
});

export async function GET(req: Request) {
  if (ghPagesEnabled) {
    return Response.json({
      query: '',
      results: [],
      note: 'Search API is disabled in GitHub Pages export mode.',
    });
  }

  return searchHandler.GET(req);
}
