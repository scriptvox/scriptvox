import Link from 'next/link';

export default function NotFound() {
  return (
    <main className="mx-auto flex min-h-[70vh] w-full max-w-[1200px] flex-col justify-center px-4 sm:px-8">
      <p className="font-mono text-sm text-[var(--sv-smoke)]">404</p>
      <h1 className="mt-4 max-w-[20ch] text-4xl sm:text-6xl font-extrabold [font-stretch:112%] tracking-[-0.02em] text-[var(--sv-caption)]">
        This page is not here.
      </h1>
      <p className="mt-6 max-w-[52ch] text-lg text-[var(--sv-smoke)]">
        The address may have a typing error, or the page moved.
      </p>
      <p className="mt-8 flex gap-6">
        <Link href="/" className="underline decoration-[var(--sv-caption)] underline-offset-4 hover:text-[var(--sv-caption)]">
          Go to the home page
        </Link>
        <Link href="/docs" className="underline decoration-[var(--sv-caption)] underline-offset-4 hover:text-[var(--sv-caption)]">
          Read the docs
        </Link>
      </p>
    </main>
  );
}
