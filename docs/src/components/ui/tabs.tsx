'use client';

import { Tabs as TabsPrimitive } from 'radix-ui';
import type * as React from 'react';
import { cn } from '@/lib/utils';

function Tabs({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Root>) {
  return <TabsPrimitive.Root className={cn('flex flex-col', className)} {...props} />;
}

function TabsList({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.List>) {
  return (
    <TabsPrimitive.List
      className={cn('flex flex-wrap border-b border-border/60 bg-muted/40', className)}
      {...props}
    />
  );
}

function TabsTrigger({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  return (
    <TabsPrimitive.Trigger
      className={cn(
        'cursor-pointer border-r border-border/50 px-4 py-2.5 text-xs font-mono tracking-normal text-muted-foreground transition last:border-r-0 hover:bg-background/70 hover:text-foreground data-active:bg-background data-active:text-foreground',
        className,
      )}
      {...props}
    />
  );
}

export { Tabs, TabsList, TabsTrigger };
