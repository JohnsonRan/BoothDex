import type { VerificationDict } from "./messages";

export const verificationEn: VerificationDict = {
  required: "Complete the verification first.",
  failedLoad:
    "The verification couldn't load. Check your connection or turn off extensions that block scripts, then try again.",
  retry: "Try again",
  refused: "The verification didn't pass. Try again.",
  passTitle: "Verify to keep browsing",
  passDesc:
    "You've made a lot of requests in a short time. Complete the verification to browse at the usual speed for the next 10 minutes.",
  notNow: "Not now",
  unavailable:
    "The verification isn't available right now. Try again in a moment.",
};
