"use client";

import { Loader2 } from "lucide-react";
import { type FormEvent, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { Centered } from "./centered";

export function Login({ onDone }: { onDone: () => void }) {
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      await api("/api/account/login", {
        method: "POST",
        json: { login, password, keep: true },
      });
      onDone();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Centered>
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>登录管理后台</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-3">
            <Input
              autoComplete="username"
              placeholder="用户名或邮箱"
              value={login}
              onChange={(e) => setLogin(e.target.value)}
              required
              aria-label="用户名或邮箱"
            />
            <Input
              autoComplete="current-password"
              type="password"
              placeholder="密码"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              aria-label="密码"
            />
            <Button type="submit" className="w-full" disabled={busy}>
              {busy && <Loader2 className="size-4 animate-spin" />}登录
            </Button>
          </form>
        </CardContent>
      </Card>
    </Centered>
  );
}
