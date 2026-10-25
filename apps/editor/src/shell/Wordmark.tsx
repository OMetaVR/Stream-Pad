/** The StreamPad mark (a 3×3 keypad with one lit key) and name. */
export function Wordmark() {
  return (
    <div className="flex items-center gap-2.5">
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden className="shrink-0">
        <rect x="0.5" y="0.5" width="19" height="19" rx="5.5" className="fill-control stroke-line-strong" />
        {[0, 1, 2].flatMap((r) =>
          [0, 1, 2].map((c) => (
            <rect
              key={`${r}${c}`}
              x={4 + c * 4.25}
              y={4 + r * 4.25}
              width="3.25"
              height="3.25"
              rx="0.9"
              className={r === 1 && c === 1 ? "fill-accent" : "fill-faint"}
            />
          )),
        )}
      </svg>
      <span className="text-md font-semibold tracking-[-0.01em] text-fg">StreamPad</span>
    </div>
  );
}
