const AGENT_LOGOS = [
  { name: 'AMP', src: '/agents/amp.svg', href: 'https://ampcode.com/' },
  { name: 'Antigravity', src: '/agents/antigravity.svg', href: 'https://antigravity.google/' },
  {
    name: 'Claude Code',
    src: '/agents/claude-code.svg',
    href: 'https://claude.com/product/claude-code',
  },
  { name: 'ClawdBot', src: '/agents/clawdbot.svg', href: 'https://clawd.bot/' },
  { name: 'Cline', src: '/agents/cline.svg', href: 'https://cline.bot/' },
  { name: 'Codex', src: '/agents/codex.svg', href: 'https://openai.com/codex' },
  { name: 'Cursor', src: '/agents/cursor.svg', href: 'https://cursor.sh' },
  { name: 'Droid', src: '/agents/droid.svg', href: 'https://factory.ai' },
  { name: 'Gemini', src: '/agents/gemini.svg', href: 'https://gemini.google.com' },
  {
    name: 'GitHub Copilot',
    src: '/agents/copilot.svg',
    href: 'https://github.com/features/copilot',
  },
  { name: 'Goose', src: '/agents/goose.svg', href: 'https://block.github.io/goose' },
  { name: 'Kilo', src: '/agents/kilo.svg', href: 'https://kilo.ai/' },
  { name: 'Kiro CLI', src: '/agents/kiro-cli.svg', href: 'https://kiro.dev/cli' },
  { name: 'OpenCode', src: '/agents/opencode.svg', href: 'https://opencode.ai/' },
  { name: 'Roo', src: '/agents/roo.svg', href: 'https://roocode.com/' },
  { name: 'Trae', src: '/agents/trae.svg', href: 'https://www.trae.ai/' },
  { name: 'VSCode', src: '/agents/vscode.svg', href: 'https://code.visualstudio.com/' },
  { name: 'Windsurf', src: '/agents/windsurf.svg', href: 'https://codeium.com/windsurf' },
];

function AgentLogoItem({
  name,
  src,
  href,
}: {
  name: string;
  src: string;
  href: string;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="flex shrink-0 items-center px-5"
      aria-label={name}
      title={name}
    >
      <img
        src={src}
        alt={name}
        className="h-[72px] w-auto object-contain grayscale transition-all duration-300 hover:grayscale-0 sm:h-[72px] lg:h-[88px]"
        loading="lazy"
      />
    </a>
  );
}

export function HomeLogoMarquee() {
  return (
    <div className="space-y-3">
      <p className="text-xs font-mono uppercase tracking-wider text-foreground/50">
        Supported by these agents
      </p>

      <div className="relative overflow-x-clip">
        <div className="pointer-events-none absolute bottom-0 left-0 top-0 z-10 w-24 bg-gradient-to-r from-background to-transparent sm:w-32 lg:w-48" />
        <div className="pointer-events-none absolute bottom-0 right-0 top-0 z-10 w-24 bg-gradient-to-l from-background to-transparent sm:w-32 lg:w-48" />
        <div>
          <div className="flex w-fit motion-reduce:animate-none animate-home-logo-marquee">
            {[0, 1].map((setIdx) => (
              <div key={setIdx} className="flex shrink-0">
                {AGENT_LOGOS.map((logo) => (
                  <AgentLogoItem
                    key={`${setIdx}-${logo.name}`}
                    name={logo.name}
                    src={logo.src}
                    href={logo.href}
                  />
                ))}
              </div>
            ))}
          </div>
        </div>

      </div>
    </div>
  );
}
