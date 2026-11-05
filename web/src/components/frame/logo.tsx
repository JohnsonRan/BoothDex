import { useId } from "react";

/** The BoothDex mark (master's logo): a speech bubble with "BD" cut out, in the text colour. */
export function Logo() {
  const id = useId();
  return (
    <svg
      viewBox="0 0 32 32"
      width={28}
      height={28}
      aria-hidden="true"
      fill="currentColor"
    >
      <mask id={id}>
        <rect width="32" height="32" fill="#fff" />
        <path
          d="M8 9v12M8 9h3.5a3 3 0 0 1 0 6H8M8 15h4a3 3 0 0 1 0 6H8M18 9v12M18 9h1.5a6 6 0 0 1 0 12H18"
          fill="none"
          stroke="#000"
          strokeWidth="3"
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
