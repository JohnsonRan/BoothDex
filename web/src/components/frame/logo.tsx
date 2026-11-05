import { useId } from "react";

/** The BoothDex mark (master's logo): a speech bubble with "BD" cut out, in the text colour; the same paths as
 *  app/icon.svg, whose strokes sit on the 16 px grid. */
export function Logo() {
  const id = useId();
  return (
    <svg
      viewBox="0 0 32 32"
      // 28 px at a 16 px root, scaling with it (globals.css)
      style={{ width: "1.75rem", height: "1.75rem" }}
      aria-hidden="true"
      fill="currentColor"
    >
      <mask id={id}>
        <rect width="32" height="32" fill="#fff" />
        <path
          d="M8 10v12M8 10h2a3 3 0 0 1 0 6H8M8 16h2.5a3 3 0 0 1 0 6H8M19 10v12M19 10h.5a6 6 0 0 1 0 12H19"
          fill="none"
          stroke="#000"
          strokeWidth="3.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </mask>
      <path
        mask={`url(#${id})`}
        d="M8 3h16a6 6 0 0 1 6 6v12a6 6 0 0 1-6 6H13l-6 4v-4.3A6 6 0 0 1 2 21V9a6 6 0 0 1 6-6Z"
      />
    </svg>
  );
}
