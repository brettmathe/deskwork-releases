export interface Toast {
  id: number;
  message: string;
  kind: "info" | "error";
}

let nextId = 1;

export const toasts = $state<{ list: Toast[] }>({ list: [] });

export function toast(message: string, kind: Toast["kind"] = "info") {
  const id = nextId++;
  toasts.list.push({ id, message, kind });
  setTimeout(() => {
    const i = toasts.list.findIndex((t) => t.id === id);
    if (i >= 0) toasts.list.splice(i, 1);
  }, kind === "error" ? 6000 : 3000);
}

export function toastError(e: unknown) {
  toast(typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e), "error");
}
