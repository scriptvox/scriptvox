import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { appName, gitConfig } from './shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: <span className="sv-wordmark">{appName}</span>,
    },
    links: [
      { text: 'Docs', url: '/docs' },
      { text: 'Releases', url: `https://github.com/${gitConfig.user}/${gitConfig.repo}/releases`, external: true },
    ],
    githubUrl: `https://github.com/${gitConfig.user}/${gitConfig.repo}`,
    themeSwitch: { enabled: false },
  };
}
