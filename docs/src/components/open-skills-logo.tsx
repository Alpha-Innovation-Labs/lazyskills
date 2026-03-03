const LETTERS: Record<string, number[][]> = {
  L: [
    [1, 0, 0, 0],
    [1, 0, 0, 0],
    [1, 0, 0, 0],
    [1, 0, 0, 0],
    [1, 1, 1, 1],
  ],
  E: [
    [1, 1, 1, 1],
    [1, 0, 0, 0],
    [1, 1, 1, 0],
    [1, 0, 0, 0],
    [1, 1, 1, 1],
  ],
  N: [
    [1, 0, 0, 1],
    [1, 1, 0, 1],
    [1, 0, 1, 1],
    [1, 0, 0, 1],
    [1, 0, 0, 1],
  ],
  A: [
    [1, 1, 1, 1],
    [1, 0, 0, 1],
    [1, 1, 1, 1],
    [1, 0, 0, 1],
    [1, 0, 0, 1],
  ],
  Z: [
    [1, 1, 1, 1],
    [0, 0, 1, 0],
    [0, 1, 0, 0],
    [1, 0, 0, 0],
    [1, 1, 1, 1],
  ],
  Y: [
    [1, 0, 0, 1],
    [1, 0, 0, 1],
    [0, 1, 1, 0],
    [0, 1, 1, 0],
    [0, 1, 1, 0],
  ],
};

const WORD = 'LAZY';

function Letter({ char }: { char: string }) {
  const grid = LETTERS[char] ?? LETTERS.L;

  return (
    <div className="relative">
      <div
        className="absolute left-[3px] top-[3px] -z-10 flex flex-col gap-[2px] sm:left-[6px] sm:top-[6px] sm:gap-[3px]"
        aria-hidden="true"
      >
        {grid.map((row, y) => (
          <div key={`shadow-row-${char}-${y}`} className="flex gap-[2px] sm:gap-[3px]">
            {row.map((cell, x) => (
              <div
                key={`shadow-cell-${char}-${y}-${x}`}
                className={`h-3 w-3 sm:h-5 sm:w-5 ${cell ? 'border border-white/80' : 'bg-transparent'}`}
              />
            ))}
          </div>
        ))}
      </div>

      <div className="relative flex flex-col gap-[2px] sm:gap-[3px]">
        {grid.map((row, y) => (
          <div key={`main-row-${char}-${y}`} className="flex gap-[2px] sm:gap-[3px]">
            {row.map((cell, x) => (
              <div
                key={`main-cell-${char}-${y}-${x}`}
                className={`h-3 w-3 sm:h-5 sm:w-5 ${cell ? 'bg-white' : 'bg-transparent'}`}
              />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

export function OpenSkillsLogo() {
  return (
    <div className="flex select-none items-end justify-center">
      <div className="flex items-end gap-1.5 sm:gap-2.5">
        {WORD.split('').map((char) => (
          <Letter key={char} char={char} />
        ))}
      </div>

      <div className="relative z-20 ml-2 mb-[-4px] sm:mb-[-8px]">
        <span className="font-caveat block -rotate-6 text-4xl text-cyan-300 opacity-0 animate-[skills-write-in_0.8s_ease-out_0.5s_forwards] sm:text-6xl">
          skills
        </span>
        <div className="absolute -right-2 top-0 h-2.5 w-2.5 rounded-full bg-cyan-300/60 animate-ping [animation-delay:1s]" />
      </div>
    </div>
  );
}
