import type { GuestbookDict } from "./messages";

export const guestbookEn: GuestbookDict = {
  title: "Guestbook",
  subtitle: "Messages are public and can't be changed after you post them.",
  nickname: "Nickname",
  body: "Message",
  count: "{n}/{max}",
  nicknameRule:
    "Your nickname must be 1 to {max} characters, with no invisible characters.",
  bodyRule:
    "Your message must be 1 to {max} characters, with no invisible characters.",
  post: "Post",
  confirmTitle: "Post this message?",
  confirmDesc: "You can't change it after you post it.",
  confirm: "Post message",
  back: "Back",
  posted: "Your message is posted",
  verify: "Complete the verification first.",
  verifyFailedLoad:
    "The verification couldn't load. Check your connection or turn off extensions that block scripts, then try again.",
  more: "Load more",
  empty: "No messages yet",
  emptyDesc: "Write the first one.",
  closed: "The guestbook isn't open yet",
  closedDesc: "You can post here once an admin opens it.",
  failed: "Messages couldn't load",
  retry: "Try again",
  "error.entry":
    "Your nickname must be 1 to 24 characters and your message 1 to 500 characters.",
  "error.card": "Remove the card number, then post again.",
  "error.verification": "The verification didn't pass. Try again.",
  "error.tooMany": "You're posting too often. Try again later.",
  "error.failed": "Something went wrong. Try again.",
};
