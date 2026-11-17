import { useState } from "react";

// The starter page of a new Vite and React app: a counter and a few links. The agent-install check
// asks an assistant to add a chart to this page and counts how many of these lines it deleted.

const page = {
  fontFamily: "system-ui, Avenir, Helvetica, Arial, sans-serif",
  lineHeight: 1.5,
  color: "#213547",
  margin: "0 auto",
  maxWidth: 960,
  padding: "2rem",
  textAlign: "center",
} as const;

const card = {
  padding: "2em",
  border: "1px solid #d0d7de",
  borderRadius: 8,
  margin: "1.5em auto",
  maxWidth: 480,
} as const;

const button = {
  borderRadius: 8,
  border: "1px solid transparent",
  padding: "0.6em 1.2em",
  fontSize: "1em",
  fontWeight: 500,
  fontFamily: "inherit",
  backgroundColor: "#f1f3f5",
  cursor: "pointer",
} as const;

const links = [
  { href: "https://vite.dev", label: "Vite" },
  { href: "https://react.dev", label: "React" },
  { href: "https://www.typescriptlang.org", label: "TypeScript" },
  { href: "https://vite.dev/guide", label: "Guide" },
];

function Logo({ color, label }: { color: string; label: string }) {
  return (
    <svg
      width="64"
      height="64"
      viewBox="0 0 64 64"
      role="img"
      aria-label={label}
      style={{ margin: "0 1em" }}
    >
      <circle cx="32" cy="32" r="28" fill="none" stroke={color} strokeWidth="4" />
      <circle cx="32" cy="32" r="6" fill={color} />
      <ellipse
        cx="32"
        cy="32"
        rx="28"
        ry="10"
        fill="none"
        stroke={color}
        strokeWidth="2"
        transform="rotate(60 32 32)"
      />
    </svg>
  );
}

// The counter keeps its count in React state, so a click draws this card again and nothing else.
// It is the part of the starter that an added chart must leave working.

function Counter() {
  const [count, setCount] = useState(0);

  return (
    <div style={card}>
      <button style={button} onClick={() => setCount((value) => value + 1)}>
        count is {count}
      </button>
      <p>
        Edit <code>src/App.tsx</code> and save to test hot module replacement.
      </p>
    </div>
  );
}

function Links() {
  return (
    <ul style={{ listStyle: "none", padding: 0 }}>
      {links.map((link) => (
        <li key={link.href} style={{ margin: "0.4em 0" }}>
          <a href={link.href} target="_blank" rel="noreferrer">
            {link.label}
          </a>
        </li>
      ))}
    </ul>
  );
}

export default function App() {
  return (
    <main style={page}>
      <div>
        <Logo color="#646cff" label="Vite logo" />
        <Logo color="#61dafb" label="React logo" />
      </div>
      <h1>Vite + React</h1>
      <Counter />
      <p>Click on the logos to learn more.</p>
      <Links />
    </main>
  );
}
