import type { Metadata } from 'next';
import { Archivo, JetBrains_Mono } from 'next/font/google';
import { Provider } from '@/components/provider';
import './global.css';

// Fonts are downloaded at build time and served from this site: no request to a font host.
const archivo = Archivo({ subsets: ['latin'], axes: ['wdth'], variable: '--font-archivo' });
const mono = JetBrains_Mono({ subsets: ['latin'], variable: '--font-mono' });

export const metadata: Metadata = {
  metadataBase: new URL('https://scriptvox.io'),
  title: { default: 'ScriptVox', template: '%s | ScriptVox' },
  description:
    'A command-line tool that turns a script and your clips into a narrated, captioned MP4. No account, no telemetry.',
};

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang="en" className={`dark ${archivo.variable} ${mono.variable}`} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
