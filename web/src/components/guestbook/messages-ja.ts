import type { GuestbookDict } from "./messages";

export const guestbookJa: GuestbookDict = {
  title: "ゲストブック",
  subtitle: "メッセージは公開され、投稿後は変更できません。",
  nickname: "ニックネーム",
  body: "メッセージ",
  bodyCount: "{n}/{max} 文字",
  nicknameRule:
    "ニックネームは見えない文字を含めず、1～{max} 文字で入力してください。",
  bodyRule:
    "メッセージは見えない文字を含めず、{lines} 行以内、1～{max} 文字で入力してください。",
  post: "投稿",
  confirmTitle: "このメッセージを投稿しますか?",
  confirmDesc: "投稿後は変更できません。",
  confirm: "投稿する",
  back: "戻る",
  posted: "メッセージを投稿しました",
  more: "さらに読み込む",
  empty: "メッセージはまだありません",
  emptyDesc: "最初のメッセージを書いてみてください。",
  closed: "ゲストブックはまだ公開されていません",
  closedDesc: "管理者が公開すると、ここに投稿できます。",
  failed: "メッセージを読み込めませんでした",
  retry: "再試行",
  "status.completed": "完了",
  "status.not_planned": "予定なし",
  "error.entry":
    "ニックネームは 1～24 文字、メッセージは 10 行以内、1～500 文字で入力してください。",
  "error.tooMany": "投稿が多すぎます。しばらくしてからもう一度お試しください。",
  "error.failed": "問題が発生しました。もう一度お試しください。",
};
