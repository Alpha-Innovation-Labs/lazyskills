import { RootProvider } from 'fumadocs-ui/provider/next';
import './global.css';
import { Caveat, Inter } from 'next/font/google';

const inter = Inter({
  subsets: ['latin'],
});

const caveat = Caveat({
  subsets: ['latin'],
  variable: '--font-caveat',
});

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang="en" className={`${inter.className} ${caveat.variable}`} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
