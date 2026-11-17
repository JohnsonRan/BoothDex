import type { VerificationDict } from "./messages";

export const verificationKo: VerificationDict = {
  required: "먼저 인증을 완료하세요.",
  failedLoad:
    "인증을 불러올 수 없어요. 연결을 확인하거나 스크립트를 차단하는 확장 프로그램을 끈 다음 다시 시도하세요.",
  retry: "다시 시도",
  refused: "인증에 실패했어요. 다시 시도하세요.",
  passTitle: "인증하고 계속 둘러보기",
  passDesc:
    "짧은 시간에 요청이 많았어요. 인증을 완료하면 앞으로 10분 동안 평소 속도로 둘러볼 수 있어요.",
  notNow: "나중에",
  unavailable: "지금은 인증을 할 수 없어요. 잠시 후 다시 시도하세요.",
};
