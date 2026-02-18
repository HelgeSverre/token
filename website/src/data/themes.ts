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
    gutterFgActive: string;
    gutterBorder: string;
    tabBarBg: string;
    tabBarActiveBg: string;
    tabBarActiveFg: string;
    tabBarInactiveBg: string;
    tabBarInactiveFg: string;
    tabBarBorder: string;
    splitterBg: string;
    punctuation: string;
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
      sidebarBg: "#181818", statusBg: "#1E1E1E", statusFg: "#787A80",
      gutterFgActive: "#BCBEC4", gutterBorder: "#2B2B2B",
      tabBarBg: "#1E1E1E", tabBarActiveBg: "#181818", tabBarActiveFg: "#FFFFFF",
      tabBarInactiveBg: "#252525", tabBarInactiveFg: "#808080", tabBarBorder: "#323232",
      splitterBg: "#252525", punctuation: "#A9B7C6",
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
    description: "VS Code-inspired dark theme",
    fileName: "dark.yaml",
    isLight: false,
    colors: {
      bg: "#1E1E1E", fg: "#D4D4D4", currentLine: "#2A2A2A", cursor: "#FCE146",
      selection: "#264F78", gutterBg: "#1E1E1E", gutterFg: "#858585",
      sidebarBg: "#252526", statusBg: "#007ACC", statusFg: "#FFFFFF",
      gutterFgActive: "#C6C6C6", gutterBorder: "#303030",
      tabBarBg: "#252526", tabBarActiveBg: "#1E1E1E", tabBarActiveFg: "#FFFFFF",
      tabBarInactiveBg: "#2D2D2D", tabBarInactiveFg: "#808080", tabBarBorder: "#3C3C3C",
      splitterBg: "#252526", punctuation: "#D4D4D4",
      keyword: "#C586C0", function: "#DCDCAA", string: "#CE9178",
      number: "#B5CEA8", comment: "#6A9955", type: "#4EC9B0",
      variable: "#9CDCFE", operator: "#D4D4D4", constant: "#569CD6",
      tag: "#569CD6", attribute: "#9CDCFE",
    },
    yaml: `name: "Default Dark"
editor:
  background: "#1E1E1E"
  foreground: "#D4D4D4"
  cursor: "#FCE146"
  selection: "#264F78"
syntax:
  keyword: "#C586C0"
  function: "#DCDCAA"
  string: "#CE9178"
  number: "#B5CEA8"
  comment: "#6A9955"`,
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
      selection: "#264F78", gutterBg: "#0D1117", gutterFg: "#484F58",
      sidebarBg: "#010409", statusBg: "#161B22", statusFg: "#8B949E",
      gutterFgActive: "#C9D1D9", gutterBorder: "#21262D",
      tabBarBg: "#161B22", tabBarActiveBg: "#0D1117", tabBarActiveFg: "#E6EDF3",
      tabBarInactiveBg: "#1C2128", tabBarInactiveFg: "#8B949E", tabBarBorder: "#21262D",
      splitterBg: "#21262D", punctuation: "#C9D1D9",
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
  selection: "#264F78"
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
      selection: "#ADD6FF", gutterBg: "#FFFFFF", gutterFg: "#8C959F",
      sidebarBg: "#F6F8FA", statusBg: "#F6F8FA", statusFg: "#57606A",
      gutterFgActive: "#24292F", gutterBorder: "#D0D7DE",
      tabBarBg: "#F6F8FA", tabBarActiveBg: "#FFFFFF", tabBarActiveFg: "#24292F",
      tabBarInactiveBg: "#EEF1F4", tabBarInactiveFg: "#57606A", tabBarBorder: "#D0D7DE",
      splitterBg: "#D0D7DE", punctuation: "#24292F",
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
  selection: "#ADD6FF"
syntax:
  keyword: "#CF222E"
  function: "#8250DF"
  string: "#0A3069"
  number: "#0550AE"
  comment: "#6E7781"`,
  },
];
