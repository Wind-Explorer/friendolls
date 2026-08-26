export type PanelState = {
  dirty: boolean;
  busy: boolean;
};

export type PanelActions = {
  apply: () => Promise<boolean>;
  reset: () => void;
};

export type RegisterPanel = (
  name: string,
  actions: PanelActions,
  state: PanelState,
) => void;
