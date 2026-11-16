import { useId } from "react";

export function LilyMark({ className }: { className?: string }) {
  const id = useId().replace(/[:]/g, "");
  const petal = `petal-${id}`;
  const core = `core-${id}`;
  const soft = `soft-${id}`;

  return (
    <svg viewBox="0 0 1024 1024" className={className} role="img" aria-label="Rawjan spider lily mark">
      <defs>
        <linearGradient id={petal} x1="512" y1="250" x2="512" y2="512" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#e2687c" stopOpacity="0.95" />
          <stop offset="55%" stopColor="#b0243a" stopOpacity="0.92" />
          <stop offset="100%" stopColor="#5c0e1b" stopOpacity="0.9" />
        </linearGradient>
        <radialGradient id={core} cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="#f7d4d9" />
          <stop offset="45%" stopColor="#c3364d" />
          <stop offset="100%" stopColor="#710f1f" />
        </radialGradient>
        <filter id={soft} x="-25%" y="-25%" width="150%" height="150%">
          <feGaussianBlur stdDeviation="6" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <g filter={`url(#${soft})`}>
        {[0, 60, 120, 180, 240, 300].map((rotation) => (
          <g key={rotation} transform={`rotate(${rotation} 512 512)`}>
            <path
              d="M512 512 C 472 440, 466 350, 500 262 C 508 242, 516 242, 524 262 C 558 350, 552 440, 512 512 Z"
              fill={`url(#${petal})`}
              stroke="#e2687c"
              strokeOpacity="0.16"
              strokeWidth="2"
            />
          </g>
        ))}
        <g stroke="#d24a5e" strokeWidth="6" strokeLinecap="round" fill="none" opacity="0.9">
          <path d="M512 512 C 494 438, 470 372, 430 318" />
          <path d="M512 512 C 530 438, 554 372, 594 318" />
          <path d="M512 512 C 472 500, 408 486, 344 482" />
          <path d="M512 512 C 552 500, 616 486, 680 482" />
          <path d="M512 512 C 486 548, 446 596, 408 648" />
          <path d="M512 512 C 538 548, 578 596, 616 648" />
        </g>
        <g fill="#e8a3af">
          <circle cx="430" cy="318" r="10" />
          <circle cx="594" cy="318" r="10" />
          <circle cx="344" cy="482" r="10" />
          <circle cx="680" cy="482" r="10" />
          <circle cx="408" cy="648" r="10" />
          <circle cx="616" cy="648" r="10" />
        </g>
        <circle cx="512" cy="512" r="36" fill={`url(#${core})`} />
        <circle cx="512" cy="512" r="36" fill="none" stroke="#f6d7dc" strokeOpacity="0.24" strokeWidth="2" />
      </g>
    </svg>
  );
}
