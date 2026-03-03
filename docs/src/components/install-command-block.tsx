'use client';

import { Check, Copy } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';

export type InstallOption = {
  id: string;
  label: string;
  command: string;
  displayLines?: string[];
};

type InstallCommandBlockProps = {
  options: InstallOption[];
};

export function InstallCommandBlock({ options }: InstallCommandBlockProps) {
  const [active, setActive] = useState(options[0]?.id ?? '');
  const [copied, setCopied] = useState(false);

  const activeInstall = useMemo(
    () => options.find((option) => option.id === active) ?? options[0],
    [active, options],
  );

  async function copyInstallCommand() {
    if (!activeInstall) return;
    await navigator.clipboard.writeText(activeInstall.command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="overflow-hidden border border-border/60 bg-card">
      <Tabs value={active} onValueChange={setActive}>
        <TabsList>
          {options.map((option) => (
            <TabsTrigger key={option.id} value={option.id}>
              {option.label}
            </TabsTrigger>
          ))}
        </TabsList>

        <div className="flex items-start gap-3 bg-background px-4 py-3">
          <span className="pt-0.5 text-emerald-500">$</span>
          <code className="flex-1 overflow-x-auto font-mono text-sm leading-6 whitespace-pre">
            {activeInstall?.displayLines?.join('\n') ?? activeInstall?.command}
          </code>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={copyInstallCommand}
            className="border border-border/60"
            aria-label="Copy install command"
          >
            {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
          </Button>
        </div>
      </Tabs>
    </div>
  );
}
