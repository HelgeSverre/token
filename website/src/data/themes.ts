export interface ThemeData {
  id: string;
  name: string;
  author: string;
  description: string;
  fileName: string;
  isLight: boolean;
  colors: {
    bg: string;
    fg: string;
    currentLine: string;
    cursor: string;
    selection: string;
    gutterBg: string;
    gutterFg: string;
    sidebarBg: string;
    statusBg: string;
    statusFg: string;
    keyword: string;
    function: string;
    string: string;
    number: string;
    comment: string;
    type: string;
    variable: string;
    operator: string;
    constant: string;
    tag: string;
    attribute: string;
  };
  yaml: string;
}

export const themes: ThemeData[] = [
  {
    id: "fleet-dark",
    name: "Fleet Dark",
    author: "Token",
    description: "Default dark theme inspired by JetBrains Fleet",
    fileName: "fleet-dark.yaml",
    isLight: false,
    colors: {
      bg: "#181818", fg: "#BCBEC4", currentLine: "#1E1E1E", cursor: "#FFFEF8",
      selection: "#214283", gutterBg: "#181818", gutterFg: "#606366",
      sidebarBg: "#181818", statusBg: "#1E1E1E", statusFg: "#606366",
      keyword: "#CC7832", function: "#FFC66D", string: "#6A8759",
      number: "#6897BB", comment: "#808080", type: "#CC7832",
      variable: "#BCBEC4", operator: "#A9B7C6", constant: "#6897BB",
      tag: "#E8BF6A", attribute: "#BABABA",
    },
    yaml: `name: "Fleet Dark"
editor:
  background: "#181818"
  foreground: "#BCBEC4"
  cursor: "#FFFEF8"
  selection: "#214283"
syntax:
  keyword: "#CC7832"
  function: "#FFC66D"
  string: "#6A8759"
  number: "#6897BB"
  comment: "#808080"`,
  },
  {
    id: "dark",
    name: "Default Dark",
    author: "Token",
    description: "A balanced dark theme with blue accents",
    fileName: "dark.yaml",
    isLight: false,
    colors: {
      bg: "#1E1E2E", fg: "#CDD6F4", currentLine: "#313244", cursor: "#F5E0DC",
      selection: "#45475A", gutterBg: "#1E1E2E", gutterFg: "#6C7086",
      sidebarBg: "#1E1E2E", statusBg: "#007ACC", statusFg: "#FFFFFF",
      keyword: "#CBA6F7", function: "#89B4FA", string: "#A6E3A1",
      number: "#FAB387", comment: "#6C7086", type: "#89DCEB",
      variable: "#CDD6F4", operator: "#94E2D5", constant: "#F38BA8",
      tag: "#89B4FA", attribute: "#F9E2AF",
    },
    yaml: `name: "Default Dark"
editor:
  background: "#1E1E2E"
  foreground: "#CDD6F4"
  cursor: "#F5E0DC"
  selection: "#45475A"
syntax:
  keyword: "#CBA6F7"
  function: "#89B4FA"
  string: "#A6E3A1"
  number: "#FAB387"
  comment: "#6C7086"`,
  },
  {
    id: "github-dark",
    name: "GitHub Dark",
    author: "Token",
    description: "GitHub's dark theme colors",
    fileName: "github-dark.yaml",
    isLight: false,
    colors: {
      bg: "#0D1117", fg: "#C9D1D9", currentLine: "#161B22", cursor: "#58A6FF",
      selection: "#1F3A5F", gutterBg: "#0D1117", gutterFg: "#484F58",
      sidebarBg: "#0D1117", statusBg: "#161B22", statusFg: "#8B949E",
      keyword: "#FF7B72", function: "#D2A8FF", string: "#A5D6FF",
      number: "#79C0FF", comment: "#8B949E", type: "#FFA657",
      variable: "#C9D1D9", operator: "#C9D1D9", constant: "#79C0FF",
      tag: "#7EE787", attribute: "#79C0FF",
    },
    yaml: `name: "GitHub Dark"
editor:
  background: "#0D1117"
  foreground: "#C9D1D9"
  cursor: "#58A6FF"
  selection: "#1F3A5F"
syntax:
  keyword: "#FF7B72"
  function: "#D2A8FF"
  string: "#A5D6FF"
  number: "#79C0FF"
  comment: "#8B949E"`,
  },
  {
    id: "github-light",
    name: "GitHub Light",
    author: "Token",
    description: "GitHub's clean light theme",
    fileName: "github-light.yaml",
    isLight: true,
    colors: {
      bg: "#FFFFFF", fg: "#24292F", currentLine: "#F6F8FA", cursor: "#0969DA",
      selection: "#B6D7FF", gutterBg: "#FFFFFF", gutterFg: "#8C959F",
      sidebarBg: "#F6F8FA", statusBg: "#F6F8FA", statusFg: "#57606A",
      keyword: "#CF222E", function: "#8250DF", string: "#0A3069",
      number: "#0550AE", comment: "#6E7781", type: "#953800",
      variable: "#24292F", operator: "#24292F", constant: "#0550AE",
      tag: "#116329", attribute: "#0550AE",
    },
    yaml: `name: "GitHub Light"
editor:
  background: "#FFFFFF"
  foreground: "#24292F"
  cursor: "#0969DA"
  selection: "#B6D7FF"
syntax:
  keyword: "#CF222E"
  function: "#8250DF"
  string: "#0A3069"
  number: "#0550AE"
  comment: "#6E7781"`,
  },
];
