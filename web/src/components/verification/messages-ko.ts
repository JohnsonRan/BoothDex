import type { VerificationDict } from "./messages";

export const verificationKo: VerificationDict = {
  required: "먼저 인증을 완료하세요.",
  failedLoad:
    "인증을 불러올 수 없어요. 연결을 확인하거나 스크립트를 차단하는 확장 프로그램을 끈 다음 다시 시도하세요.",
  retry: "다시 시도",
  refused: "인증에 실패했어요. 다시 시도하세요.",
};
