// Settings › About › Updates: compare with the latest GitHub release, read its notes, install it in place.

import { ArrowUpCircle, CheckCircle2, Download, ExternalLink, RefreshCw, RotateCw, TriangleAlert } from "lucide-react";
import { useEffect } from "react";
import { getBackend, type InstallMethod } from "@/lib/backend";
import { checkForUpdate, installUpdate, setAutoCheck, useUpdates } from "@/state/updates";
import { Badge, Button, Card, Field, Section, Switch } from "@/ui";

const METHOD_HINT: Record<InstallMethod, string> = {
  pacman: "Installs the new package (you'll be asked for your password) and restarts StreamPad.",
  user: "Downloads and installs the new version for your user and restarts StreamPad — no password needed.",
  manual: "This copy wasn't installed from a release (e.g. built from source), so update it the way you installed it.",
};

function relative(iso: string): string {
  const days = Math.round((Date.now() - new Date(iso).getTime()) / 86_400_000);
  if (!Number.isFinite(days)) return "";
  return days <= 0 ? "today" : days === 1 ? "yesterday" : `${days} days ago`;
}

/** Just enough Markdown for release notes: headings, bullets, paragraphs, **bold** and `code`. */
function Notes({ text }: { text: string }) {
  const inline = (s: string) =>
    s.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((part, i) =>
      part.startsWith("**") ? (
        <strong key={i} className="font-semibold text-fg">
          {part.slice(2, -2)}
        </strong>
      ) : part.startsWith("`") ? (
        <code key={i} className="font-code text-accent-text">
          {part.slice(1, -1)}
        </code>
      ) : (
        part
      ),
    );
  return (
    <div className="flex max-h-64 flex-col gap-1 overflow-y-auto text-sm text-muted">
      {text
        .replace(/\r/g, "")
        .split("\n")
        .filter((l, i, all) => l.trim() || (all[i - 1]?.trim() ?? ""))
        .map((line, i) => {
          const h = /^#{1,6}\s+(.*)/.exec(line);
          if (h) return <p key={i} className="mt-2 text-base font-semibold text-fg first:mt-0">{inline(h[1]!)}</p>;
          const li = /^\s*[-*]\s+(.*)/.exec(line);
          if (li) return <p key={i} className="pl-3 before:-ml-3 before:mr-1.5 before:content-['•']">{inline(li[1]!)}</p>;
          if (/^\s*```/.test(line)) return null;
          return line.trim() ? <p key={i}>{inline(line)}</p> : <div key={i} className="h-1" />;
        })}
    </div>
  );
}

export function UpdatesSection() {
  const { info, checking, checkError, checkedAt, auto, install } = useUpdates();
  useEffect(() => {
    if (!info && !checking) void checkForUpdate();
    // Check once when the section opens; later checks are explicit.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const running = install.kind === "running";
  const progress = running ? install.progress : null;
  const pct = progress?.phase === "download" && progress.total ? Math.round((progress.received / progress.total) * 100) : null;
  const canInstall = !!info?.available && info.method !== "manual" && !!info.asset;

  return (
    <Section
      title="Updates"
      actions={
        <Button size="sm" variant="ghost" icon={RefreshCw} loading={checking} disabled={running} onClick={() => void checkForUpdate()}>
          Check now
        </Button>
      }
    >
      <Card divided>
        <div className="flex items-start gap-3 px-4 py-4">
          {checkError ? (
            <TriangleAlert size={20} className="mt-0.5 shrink-0 text-warning" />
          ) : info?.available && install.kind !== "installed" ? (
            <ArrowUpCircle size={20} className="mt-0.5 shrink-0 text-accent-text" />
          ) : (
            <CheckCircle2 size={20} className="mt-0.5 shrink-0 text-success" />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-base font-medium">
                {install.kind === "installed"
                  ? `StreamPad ${install.version} is installed`
                  : checkError
                    ? "Couldn't check for updates"
                    : !info
                      ? "Checking for updates…"
                      : info.available
                        ? `StreamPad ${info.latest} is available`
                        : "StreamPad is up to date"}
              </span>
              <Badge>v{getBackend().appVersion()}</Badge>
            </div>
            <p className="mt-0.5 text-sm text-muted">
              {install.kind === "installed"
                ? "The background service already runs the new version. Restart the editor to finish."
                : checkError
                  ? checkError
                  : info?.available
                    ? `Released ${relative(info.publishedAt)}. ${METHOD_HINT[info.method]}`
                    : checkedAt
                      ? `Last checked ${new Date(checkedAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}.`
                      : "Looking at the latest release on GitHub."}
            </p>

            {info?.available && install.kind !== "installed" && info.notes.trim() && (
              <div className="mt-3 rounded-md border border-line-subtle bg-raised px-3 py-2.5">
                <Notes text={info.notes} />
              </div>
            )}

            {running && (
              <div className="mt-3">
                <div className="h-1.5 overflow-hidden rounded-full bg-control">
                  <div
                    className="h-full rounded-full bg-accent transition-[width] duration-200"
                    style={{ width: progress?.phase === "download" ? `${pct ?? 0}%` : progress?.phase ? "100%" : "4%" }}
                  />
                </div>
                <p className="mt-1.5 text-xs text-muted">
                  {progress?.phase === "install"
                    ? "Installing…"
                    : progress?.phase === "download"
                      ? `Downloading… ${(progress.received / 1e6).toFixed(1)} / ${(progress.total / 1e6).toFixed(1)} MB`
                      : "Starting…"}
                </p>
              </div>
            )}
            {install.kind === "error" && <p className="mt-2 rounded-sm bg-danger-soft px-2 py-1.5 text-sm text-danger">{install.message}</p>}

            <div className="mt-3 flex flex-wrap gap-2">
              {install.kind === "installed" ? (
                <Button variant="primary" size="sm" icon={RotateCw} onClick={() => void getBackend().restartEditor()}>
                  Restart the editor
                </Button>
              ) : (
                canInstall && (
                  <Button variant="primary" size="sm" icon={Download} loading={running} onClick={() => void installUpdate()}>
                    {install.kind === "error" ? "Try again" : `Update to ${info!.latest}`}
                  </Button>
                )
              )}
              {info?.url && (info.available || checkError) && (
                <Button size="sm" variant="secondary" icon={ExternalLink} onClick={() => void getBackend().openPath(info.url)}>
                  Release page
                </Button>
              )}
            </div>
          </div>
        </div>
        <Field label="Check for updates automatically" hint="Once a day when the editor opens." inline>
          <Switch checked={auto} onCheckedChange={setAutoCheck} />
        </Field>
      </Card>
    </Section>
  );
}
