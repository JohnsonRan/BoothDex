import { expect, test } from "bun:test";
import { fetchAccount } from "./api";

test("fetchAccount shares startup requests, allows fresh bypass and starts anew once settled", async () => {
  const originalFetch = globalThis.fetch;
  let calls = 0;
  type Deferred = {
    resolve: (res: Response) => void;
    reject: (err: unknown) => void;
  };
  const deferreds: Deferred[] = [];

  globalThis.fetch = (async (
    _input: RequestInfo | URL,
    _init?: RequestInit,
  ) => {
    calls++;
    return new Promise<Response>((resolve, reject) => {
      deferreds.push({ resolve, reject });
    });
  }) as typeof fetch;

  try {
    // (a) two plain calls share one request
    const p1 = fetchAccount<{ id: number }>();
    const p2 = fetchAccount<{ id: number }>();
    expect(calls).toBe(1);
    expect(deferreds).toHaveLength(1);

    // (b) a fresh call while a request is pending starts a new request and gets the new response
    const pFresh = fetchAccount<{ id: number }>({ fresh: true });
    expect(calls).toBe(2);
    expect(deferreds).toHaveLength(2);

    deferreds[0].resolve(
      new Response(JSON.stringify({ id: 1 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    deferreds[1].resolve(
      new Response(JSON.stringify({ id: 2 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    const [r1, r2, rFresh] = await Promise.all([p1, p2, pFresh]);
    expect(r1).toEqual({ id: 1 });
    expect(r2).toEqual({ id: 1 });
    expect(rFresh).toEqual({ id: 2 });

    // (c) after a request settles, the next plain call starts a new request
    const p3 = fetchAccount<{ id: number }>();
    expect(calls).toBe(3);
    expect(deferreds).toHaveLength(3);

    deferreds[2].resolve(
      new Response(JSON.stringify({ id: 3 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const r3 = await p3;
    expect(r3).toEqual({ id: 3 });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("superseded request settling does not clear newer pending fresh request", async () => {
  const originalFetch = globalThis.fetch;
  let calls = 0;
  type Deferred = {
    resolve: (res: Response) => void;
    reject: (err: unknown) => void;
  };
  const deferreds: Deferred[] = [];

  globalThis.fetch = (async (
    _input: RequestInfo | URL,
    _init?: RequestInit,
  ) => {
    calls++;
    return new Promise<Response>((resolve, reject) => {
      deferreds.push({ resolve, reject });
    });
  }) as typeof fetch;

  try {
    // start A
    const pA = fetchAccount<{ id: number }>();
    expect(calls).toBe(1);

    // start fresh B
    const pB = fetchAccount<{ id: number }>({ fresh: true });
    expect(calls).toBe(2);

    // settle A first
    deferreds[0].resolve(
      new Response(JSON.stringify({ id: 1 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const rA = await pA;
    expect(rA).toEqual({ id: 1 });

    // a plain call while B is pending must share B (fetch called exactly twice)
    const pC = fetchAccount<{ id: number }>();
    expect(calls).toBe(2);

    // settle B
    deferreds[1].resolve(
      new Response(JSON.stringify({ id: 2 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const [rB, rC] = await Promise.all([pB, pC]);
    expect(rB).toEqual({ id: 2 });
    expect(rC).toEqual({ id: 2 });
  } finally {
    globalThis.fetch = originalFetch;
  }
});
