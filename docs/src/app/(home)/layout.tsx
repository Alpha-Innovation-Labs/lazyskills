import { Footer } from '@/components/footer';
import { Navbar } from '@/components/navbar';

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <div className="flex min-h-screen flex-col bg-background">
      <Navbar />

      <div className="flex-1">{children}</div>

      <Footer />
    </div>
  );
}
