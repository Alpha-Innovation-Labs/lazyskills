import { HomeNavbar } from '@/components/home-navbar';

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <div className="min-h-screen bg-background">
      <HomeNavbar />
      {children}
    </div>
  );
}
