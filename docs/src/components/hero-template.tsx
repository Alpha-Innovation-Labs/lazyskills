import { InstallCommandBlock } from '@/components/install-command-block';

const INSTALL_OPTIONS = [
  {
    id: 'npm',
    label: 'npx',
    command: 'npx lazyskills',
  },
  {
    id: 'cargo',
    label: 'Cargo',
    command: 'cargo install lazyskills',
  },
  {
    id: 'brew',
    label: 'Homebrew',
    command: 'brew install alpha-innovation-labs/tap/lazyskills',
  },
  {
    id: 'scoop',
    label: 'Scoop',
    command: 'scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket && scoop install lazyskills',
    displayLines: [
      'scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket',
      'scoop install lazyskills',
    ],
  },
];

export function HeroTemplate() {

  return (
    <section className="flex-1 bg-background">
      <div className="mx-auto w-full max-w-5xl px-6 pb-16 pt-8 sm:pb-24 sm:pt-12 lg:pb-28 lg:pt-14">
        <div className="space-y-6 p-8 sm:p-10">
          <InstallCommandBlock options={INSTALL_OPTIONS} />

          <div className="mt-8 overflow-hidden sm:mt-10">
            <img
              src="/media/lazyskills-demo.gif"
              alt="Lazyskills demo showing skill discovery in the terminal UI"
              className="w-full"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
