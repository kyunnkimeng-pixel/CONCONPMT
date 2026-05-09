import { useAppStore } from "@/app/app-store";

export function AppShell() {
  const productName = useAppStore((state) => state.productName);

  return (
    <main className="min-h-screen bg-background text-foreground">
      <section className="flex min-h-screen items-center justify-center px-6">
        <div className="flex flex-col items-center gap-4 text-center">
          <p className="text-sm font-medium text-muted">Windows 데스크톱 앱 스캐폴드</p>
          <h1 className="text-4xl font-semibold tracking-normal sm:text-5xl">
            {productName}
          </h1>
          <p className="max-w-xl text-base leading-7 text-muted">
            Tauri 2, React, TypeScript, Vite, Tailwind CSS v4 기반이 준비되었습니다.
          </p>
        </div>
      </section>
    </main>
  );
}
