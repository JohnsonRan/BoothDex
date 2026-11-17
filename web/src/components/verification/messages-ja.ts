import type { VerificationDict } from "./messages";

export const verificationJa: VerificationDict = {
  required: "先に認証を完了してください。",
  failedLoad:
    "認証を読み込めませんでした。接続を確認するか、スクリプトをブロックする拡張機能をオフにしてから、もう一度お試しください。",
  retry: "再試行",
  refused: "認証に失敗しました。もう一度お試しください。",
  passTitle: "認証して閲覧を続ける",
  passDesc:
    "短時間に多くのリクエストがありました。認証を完了すると、この先 10 分間は通常の速さで閲覧できます。",
  notNow: "後で",
  unavailable:
    "現在、認証を利用できません。しばらくしてからもう一度お試しください。",
};
