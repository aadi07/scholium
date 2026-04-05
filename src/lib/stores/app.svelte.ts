import type { Document, Project, Thread, TextSelection } from '$lib/types';

// Svelte 5 class-based reactive store.
// Shape mirrors the DB schema so wiring up Rust commands later is mechanical.

class AppState {
  // Active document (the PDF currently open in the viewer)
  activeDocument = $state<Document | null>(null);

  // Active project — null until project management is built (step 8)
  activeProject = $state<Project | null>(null);

  // Projects list — populated once we have the DB layer
  projects = $state<Project[]>([]);

  // Documents for the active project
  documents = $state<Document[]>([]);

  // Active thread shown in the right panel
  activeThread = $state<Thread | null>(null);

  // Threads anchored to the active document
  threads = $state<Thread[]>([]);

  // The current text selection (drives the "Ask AI" popover)
  activeSelection = $state<TextSelection | null>(null);
}

export const app = new AppState();
