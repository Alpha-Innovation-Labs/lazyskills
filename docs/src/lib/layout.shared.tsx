import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { BookOpen, Github } from 'lucide-react';
import { Logo } from '@/components/logo';

// fill this with your actual GitHub info, for example:
export const gitConfig = {
  user: 'Alpha-Innovation-Labs',
  repo: 'lazyskills',
  branch: 'main',
};

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: <Logo className="h-5" />,
    },
    links: [
      {
        type: 'button',
        text: 'Docs',
        icon: <BookOpen className="h-4 w-4" />,
        url: '/docs',
        active: 'nested-url',
        on: 'nav',
      },
      {
        type: 'button',
        text: 'Repo',
        icon: <Github className="h-4 w-4" />,
        url: `https://github.com/${gitConfig.user}/${gitConfig.repo}`,
        external: true,
        active: 'none',
        on: 'nav',
      },
    ],
  };
}

export function homeOptions(): BaseLayoutProps {
  return {
    ...baseOptions(),
    searchToggle: {
      enabled: false,
    },
  };
}
