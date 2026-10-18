import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement>;

const base = (props: P) => ({
  width: 18,
  height: 18,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  ...props,
});

export const PowerIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M12 3v9" />
    <path d="M6.3 6.3a8 8 0 1 0 11.4 0" />
  </svg>
);

export const SnowIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M12 2v20M4.9 7l14.2 10M4.9 17L19.1 7" />
    <path d="M9 4l3 2 3-2M9 20l3-2 3 2" />
  </svg>
);

export const SunIcon = (p: P) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  </svg>
);

export const DropIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M12 2.7s6 6.4 6 11.3a6 6 0 0 1-12 0c0-4.9 6-11.3 6-11.3z" />
  </svg>
);

export const FanIcon = (p: P) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="1.6" />
    <path d="M12 10.4C11 6 12.5 3 15 3c2 0 2.6 2.6.6 4.4L13.4 9.6" />
    <path d="M13.6 12c4.4-1 7.4.5 7.4 3 0 2-2.6 2.6-4.4.6l-2.2-2.2" />
    <path d="M12 13.6c1 4.4-.5 7.4-3 7.4-2 0-2.6-2.6-.6-4.4l2.2-2.2" />
    <path d="M10.4 12C6 13 3 11.5 3 9c0-2 2.6-2.6 4.4-.6l2.2 2.2" />
  </svg>
);

export const AutoIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M4 20L10.5 4h3L20 20" />
    <path d="M7 13h10" />
  </svg>
);

export const SwingIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M4 7c4-3 12-3 16 0" />
    <path d="M4 12h16" />
    <path d="M4 17c4 3 12 3 16 0" />
  </svg>
);

export const LeafIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M5 19c0-9 5-14 15-14 0 10-5 15-14 15" />
    <path d="M5 19l8-8" />
  </svg>
);

export const BoltIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M13 2L4 14h7l-1 8 9-12h-7z" />
  </svg>
);

export const GearIcon = (p: P) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </svg>
);

export const HomeIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M3 10.5L12 3l9 7.5V21H3z" />
  </svg>
);

export const CloudIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M17.5 19H7a5 5 0 1 1 1.1-9.9A6 6 0 0 1 19.6 11 4 4 0 0 1 17.5 19z" />
  </svg>
);

export const MinusIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M5 12h14" />
  </svg>
);

export const PlusIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M12 5v14M5 12h14" />
  </svg>
);
