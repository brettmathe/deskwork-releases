import { invoke } from "@tauri-apps/api/core";

export interface InboxItem {
  filename: string;
  title: string;
  added: string | null;
  isSlack: boolean;
  preview: string;
}

export interface InboxItemFull {
  filename: string;
  title: string;
  added: string | null;
  isSlack: boolean;
  content: string;
}

export interface ExtraEntry {
  name: string;
  isDir: boolean;
}

export interface ProjectSummary {
  dir: string;
  name: string;
  priority: number | null;
  registryStatus: string | null;
  stakeholders: string | null;
  target: string | null;
  fileStatus: string | null;
  onHold: boolean;
  unregistered: boolean;
}

export interface MetaEntry {
  key: string;
  value: string;
}

export interface Artifact {
  relPath: string;
  kind: "md" | "html" | "other";
}

export interface ProjectDetail {
  dir: string;
  name: string;
  readme: string;
  metadata: MetaEntry[];
  artifacts: Artifact[];
}

export interface GitChange {
  status: "added" | "modified" | "deleted" | "renamed" | "untracked" | "other";
  path: string;
}

export interface GitStatus {
  branch: string;
  hasUpstream: boolean;
  ahead: number;
  behind: number;
  taskChanges: GitChange[];
  otherChanges: number;
  fetchError: string | null;
}

export interface RepoCandidate {
  path: string;
  hasGit: boolean;
  remote: string | null;
}

export interface Tooling {
  git: boolean;
  gh: boolean;
  ghAuthed: boolean;
  gitIdentity: boolean;
  defaultParent: string;
}

export interface SetupResult {
  path: string;
  committed: boolean;
  warning: string | null;
}

export const api = {
  getRepoRoot: () => invoke<string | null>("get_repo_root"),
  setRepoRoot: (path: string) => invoke<string>("set_repo_root", { path }),

  detectRepos: () => invoke<RepoCandidate[]>("detect_repos"),
  checkTooling: () => invoke<Tooling>("check_tooling"),
  cloneRepo: (parent: string, url: string, folder: string) =>
    invoke<SetupResult>("clone_repo", { parent, url, folder }),
  createRepo: (parent: string, folder: string, initRepo: boolean) =>
    invoke<SetupResult>("create_repo", { parent, folder, initRepo }),

  listInbox: () => invoke<InboxItem[]>("list_inbox"),
  listInboxExtras: () => invoke<ExtraEntry[]>("list_inbox_extras"),
  getInboxItem: (filename: string) => invoke<InboxItemFull>("get_inbox_item", { filename }),
  createInboxItem: (title: string, body: string) =>
    invoke<InboxItem>("create_inbox_item", { title, body }),
  saveInboxItem: (filename: string, content: string) =>
    invoke<InboxItem>("save_inbox_item", { filename, content }),
  completeInboxItem: (filename: string) => invoke<string>("complete_inbox_item", { filename }),
  deleteInboxItem: (filename: string) => invoke<void>("delete_inbox_item", { filename }),
  openInboxExtra: (name: string) => invoke<void>("open_inbox_extra", { name }),

  listProjects: () => invoke<ProjectSummary[]>("list_projects"),
  getProject: (dir: string) => invoke<ProjectDetail>("get_project", { dir }),
  readArtifact: (dir: string, relPath: string) =>
    invoke<string>("read_artifact", { dir, relPath }),
  openProjectFile: (dir: string, relPath: string) =>
    invoke<void>("open_project_file", { dir, relPath }),
  revealProjectFile: (dir: string, relPath: string) =>
    invoke<void>("reveal_project_file", { dir, relPath }),

  gitStatus: (fetch: boolean) => invoke<GitStatus>("git_status", { fetch }),
  gitPull: () => invoke<string>("git_pull"),
  gitCommitPush: (message: string) => invoke<string>("git_commit_push", { message }),
};
