"use client";

import { type FormEvent, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";

type Ai = {
  base_url?: string | null;
  model?: string | null;
  api_key_set: boolean;
  api_key_hint?: string | null;
  reasoning_effort?: string;
  embed_model?: string | null;
  from_env?: {
    base_url: boolean;
    model: boolean;
    api_key: boolean;
    reasoning_effort?: boolean;
    embed_model?: boolean;
  };
};
const EFFORTS = [
  ["none", "不思考"],
  ["minimal", "最少"],
  ["low", "低"],
  ["medium", "中"],
  ["high", "高"],
  ["xhigh", "最高"],
] as const;
const ENV_NOTE = "(由服务器环境变量设置，此处不能修改)";

/** OpenAI-compatible provider used by the enrichment job. The key is write-only: the API only returns a hint. */
export function AiSettingsCard() {
  const [ai, setAi] = useState<Ai | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [embedModel, setEmbedModel] = useState("");
  const [key, setKey] = useState("");
  const [effort, setEffort] = useState("none");
  const [busy, setBusy] = useState(false);
  const [models, setModels] = useState<string[]>([]);

  useEffect(() => {
    api<{ ai: Ai }>("/api/admin/settings").then(({ ai }) => {
      setAi(ai);
      setBaseUrl(ai.base_url ?? "");
      setModel(ai.model ?? "");
      setEmbedModel(ai.embed_model ?? "");
      setEffort(ai.reasoning_effort ?? "none");
    });
  }, []);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      setAi(
        await api<Ai>("/api/admin/settings/ai", {
          method: "PUT",
          json: {
            base_url: baseUrl,
            model,
            api_key: key,
            reasoning_effort: effort,
            embed_model: embedModel,
          },
        }),
      );
      setKey("");
      toast.success("已保存");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };
  const test = async () => {
    setBusy(true);
    try {
      const r = await api<{
        models: string[];
        model_found: boolean;
        embed_error?: string | null;
      }>("/api/admin/settings/ai/test", { method: "POST" });
      setModels(r.models);
      if (r.embed_error)
        toast.error(`向量模型不可用：${r.embed_error}`, { duration: 15000 });
      else if (r.model_found)
        toast.success(`连接成功，可用 AI 模型 ${r.models.length} 个`);
      else toast.warning("连接成功，但没找到你填的 AI 模型");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>AI 处理设置</CardTitle>
      </CardHeader>
      <CardContent>
        <form onSubmit={save} className="grid gap-3 md:grid-cols-[1fr_1fr]">
          <label htmlFor="ai-base" className="space-y-1.5 text-sm">
            <span className="text-muted-foreground">
              API 地址(OpenAI 兼容){ai?.from_env?.base_url && ENV_NOTE}
            </span>
            <Input
              id="ai-base"
              disabled={ai?.from_env?.base_url}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.openai.com/v1"
            />
          </label>
          <label htmlFor="ai-model" className="space-y-1.5 text-sm">
            <span className="text-muted-foreground">
              AI 模型{ai?.from_env?.model && ENV_NOTE}
            </span>
            <Input
              id="ai-model"
              disabled={ai?.from_env?.model}
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="gpt-4o-mini"
              list="ai-models"
            />
            <datalist id="ai-models">
              {models.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          </label>
          <div className="space-y-1.5 text-sm">
            <span id="ai-effort" className="text-muted-foreground">
              思考等级{ai?.from_env?.reasoning_effort && ENV_NOTE}
            </span>
            <Select
              value={effort}
              onValueChange={setEffort}
              disabled={ai?.from_env?.reasoning_effort}
            >
              <SelectTrigger className="w-full" aria-labelledby="ai-effort">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {EFFORTS.map(([v, label]) => (
                  <SelectItem key={v} value={v}>
                    {v}({label})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-muted-foreground text-xs">
              作为 reasoning_effort 原样发送。服务商不支持时，AI
              任务会停止并显示错误。
            </p>
          </div>
          <label htmlFor="ai-embed" className="space-y-1.5 text-sm">
            <span className="text-muted-foreground">
              向量模型(语义搜索){ai?.from_env?.embed_model && ENV_NOTE}
            </span>
            <Input
              id="ai-embed"
              disabled={ai?.from_env?.embed_model}
              value={embedModel}
              onChange={(e) => setEmbedModel(e.target.value)}
              placeholder="Qwen/Qwen3-Embedding-4B"
              list="ai-models"
            />
            <p className="text-muted-foreground text-xs">
              使用上面的 API 地址和密钥。模型需要支持输出 1024
              维向量。留空则不使用语义搜索；更换模型后会重新生成所有向量。
            </p>
          </label>
          <label htmlFor="ai-key" className="space-y-1.5 text-sm">
            <span className="text-muted-foreground">
              API 密钥{" "}
              {ai?.from_env?.api_key
                ? `${ai.api_key_hint} ${ENV_NOTE}`
                : ai?.api_key_set &&
                  `(已保存 ${ai.api_key_hint}，留空则不修改)`}
            </span>
            <Input
              id="ai-key"
              disabled={ai?.from_env?.api_key}
              type="password"
              autoComplete="off"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={ai?.api_key_set ? "••••••••" : "sk-..."}
            />
          </label>
          <div className="flex gap-2 md:col-span-2">
            <Button type="submit" disabled={busy}>
              保存
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={busy || !ai?.api_key_set}
              onClick={test}
            >
              测试连接
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
