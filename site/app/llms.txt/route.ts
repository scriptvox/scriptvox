import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { docsLlms } from '@/lib/source';

export const revalidate = false;

// The repo's own llms.txt (one source of truth), then the list of doc pages of this site.
export async function GET() {
  const repo = await readFile(path.join(process.cwd(), '..', 'llms.txt'), 'utf8');
  const docs = await (await docsLlms.serve('/llms.txt'))!.text();
  return new Response(`${repo.trimEnd()}\n\n## Site docs\n\n${docs.replace(/^# Docs\n+/, '')}`, {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  });
}
