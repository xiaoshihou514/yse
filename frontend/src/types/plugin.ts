export type PluginComponent = ListComponent | ProgressComponent;

export interface ListComponent {
  type: "list";
  id?: string;
  title?: string;
  options: Array<{
    label: string;
    value: string;
    description?: string;
  }>;
}

export interface ProgressComponent {
  type: "progress";
  id?: string;
  value: number;
  max?: number;
  status?: string;
}

export interface PluginMeta {
  plugin?: {
    component?: PluginComponent;
    response?: { component_id?: string; value: string };
  };
}
