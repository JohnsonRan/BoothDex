"use client";

import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ApiError, api } from "@/lib/api";

/** Sets a random password for a user (no mail service): the admin passes it on. Signs the user out everywhere. */
export function ResetPasswordCard() {
  const [username, setUsername] = useState("");
  const [armed, setArmed] = useState(false);
  const [result, setResult] = useState<{
    username: string;
    password: string;
    sessions_revoked: number;
  } | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    // irreversible for the user: confirm with a second click
    if (!armed) return setArmed(true);
    setBusy(true);
    try {
      setResult(
        await api("/api/admin/users/reset-password", {
          method: "POST",
          json: { username },
        }),
      );
      setUsername("");
    } catch (err) {
      toast.error(
        err instanceof ApiError && err.status === 404
          ? "找不到这个用户"
          : err instanceof Error
            ? err.message
            : String(err),
      );
    } finally {
      setBusy(false);
      setArmed(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>重置用户密码</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <p className="text-sm text-muted-foreground">
          为忘记密码的用户生成一个随机新密码，并退出该用户在所有设备上的登录。新密码只显示这一次，请转告用户，并提醒对方登录后修改。
        </p>
        <form onSubmit={submit} className="flex flex-wrap gap-2">
          <Input
            value={username}
            onChange={(e) => {
              setUsername(e.target.value);
              setArmed(false);
            }}
            placeholder="用户名"
            aria-label="用户名"
            required
            className="max-w-xs"
          />
          <Button
            type="submit"
            variant={armed ? "destructive" : "outline"}
            disabled={busy || !username.trim()}
          >
            {armed ? `确认重置 ${username.trim()} 的密码` : "重置密码"}
          </Button>
        </form>
        {result && (
          <div className="flex flex-wrap items-center gap-3 rounded-lg border border-[var(--success)]/30 bg-[var(--success)]/10 p-3 text-sm">
            <span>
              {result.username} 的新密码：
              <code className="ml-1 rounded bg-background px-2 py-0.5 font-mono text-base select-all">
                {result.password}
              </code>
            </span>
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                navigator.clipboard
                  .writeText(result.password)
                  .then(() => toast.success("已复制"))
              }
            >
              复制
            </Button>
            <span className="text-xs text-muted-foreground">
              已退出 {result.sessions_revoked} 个登录会话
            </span>
            <Button
              size="sm"
              variant="ghost"
              className="ml-auto"
              onClick={() => setResult(null)}
            >
              隐藏
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
