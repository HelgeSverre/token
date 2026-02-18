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
  {
    id: "dracula",
    name: "Dracula",
    author: "Token",
    description: "The iconic dark theme with bold purples, pinks, and cyans",
    fileName: "dracula.yaml",
    isLight: false,
    colors: {
      bg: "#282A36", fg: "#F8F8F2", currentLine: "#44475A", cursor: "#F8F8F2",
      selection: "#44475A", gutterBg: "#282A36", gutterFg: "#6272A4",
      sidebarBg: "#21222C", statusBg: "#191A21", statusFg: "#F8F8F2",
      gutterFgActive: "#F8F8F2", gutterBorder: "#343746",
      tabBarBg: "#21222C", tabBarActiveBg: "#282A36", tabBarActiveFg: "#F8F8F2",
      tabBarInactiveBg: "#21222C", tabBarInactiveFg: "#6272A4", tabBarBorder: "#343746",
      splitterBg: "#343746", punctuation: "#F8F8F2",
      keyword: "#FF79C6", function: "#50FA7B", string: "#F1FA8C",
      number: "#BD93F9", comment: "#6272A4", type: "#8BE9FD",
      variable: "#F8F8F2", operator: "#FF79C6", constant: "#BD93F9",
      tag: "#FF79C6", attribute: "#50FA7B",
    },
    yaml: `name: "Dracula"
editor:
  background: "#282A36"
  foreground: "#F8F8F2"
  cursor: "#F8F8F2"
  selection: "#44475A"
syntax:
  keyword: "#FF79C6"
  function: "#50FA7B"
  string: "#F1FA8C"
  number: "#BD93F9"
  comment: "#6272A4"`,
  },
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    author: "Token",
    description: "Soothing pastel theme with warm, cozy colors",
    fileName: "catppuccin-mocha.yaml",
    isLight: false,
    colors: {
      bg: "#1E1E2E", fg: "#CDD6F4", currentLine: "#28283D", cursor: "#F5E0DC",
      selection: "#45475A", gutterBg: "#1E1E2E", gutterFg: "#6C7086",
      sidebarBg: "#181825", statusBg: "#181825", statusFg: "#BAC2DE",
      gutterFgActive: "#CDD6F4", gutterBorder: "#313244",
      tabBarBg: "#181825", tabBarActiveBg: "#1E1E2E", tabBarActiveFg: "#CDD6F4",
      tabBarInactiveBg: "#181825", tabBarInactiveFg: "#6C7086", tabBarBorder: "#313244",
      splitterBg: "#313244", punctuation: "#BAC2DE",
      keyword: "#CBA6F7", function: "#89B4FA", string: "#A6E3A1",
      number: "#FAB387", comment: "#6C7086", type: "#F9E2AF",
      variable: "#CDD6F4", operator: "#94E2D5", constant: "#FAB387",
      tag: "#CBA6F7", attribute: "#89DCEB",
    },
    yaml: `name: "Catppuccin Mocha"
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
    id: "nord",
    name: "Nord",
    author: "Token",
    description: "Arctic, north-bluish color palette with dimmed pastels",
    fileName: "nord.yaml",
    isLight: false,
    colors: {
      bg: "#2E3440", fg: "#D8DEE9", currentLine: "#3B4252", cursor: "#D8DEE9",
      selection: "#434C5E", gutterBg: "#2E3440", gutterFg: "#4C566A",
      sidebarBg: "#2E3440", statusBg: "#3B4252", statusFg: "#D8DEE9",
      gutterFgActive: "#D8DEE9", gutterBorder: "#3B4252",
      tabBarBg: "#2E3440", tabBarActiveBg: "#3B4252", tabBarActiveFg: "#ECEFF4",
      tabBarInactiveBg: "#2E3440", tabBarInactiveFg: "#4C566A", tabBarBorder: "#3B4252",
      splitterBg: "#3B4252", punctuation: "#ECEFF4",
      keyword: "#81A1C1", function: "#88C0D0", string: "#A3BE8C",
      number: "#B48EAD", comment: "#4C566A", type: "#8FBCBB",
      variable: "#D8DEE9", operator: "#81A1C1", constant: "#5E81AC",
      tag: "#81A1C1", attribute: "#8FBCBB",
    },
    yaml: `name: "Nord"
editor:
  background: "#2E3440"
  foreground: "#D8DEE9"
  cursor: "#D8DEE9"
  selection: "#434C5E"
syntax:
  keyword: "#81A1C1"
  function: "#88C0D0"
  string: "#A3BE8C"
  number: "#B48EAD"
  comment: "#4C566A"`,
  },
  {
    id: "tokyo-night",
    name: "Tokyo Night",
    author: "Token",
    description: "Neon lights of downtown Tokyo at night",
    fileName: "tokyo-night.yaml",
    isLight: false,
    colors: {
      bg: "#1A1B26", fg: "#A9B1D6", currentLine: "#24283B", cursor: "#C0CAF5",
      selection: "#33467C", gutterBg: "#1A1B26", gutterFg: "#3B4261",
      sidebarBg: "#15161E", statusBg: "#15161E", statusFg: "#7982A9",
      gutterFgActive: "#A9B1D6", gutterBorder: "#24283B",
      tabBarBg: "#15161E", tabBarActiveBg: "#1A1B26", tabBarActiveFg: "#C0CAF5",
      tabBarInactiveBg: "#15161E", tabBarInactiveFg: "#565F89", tabBarBorder: "#24283B",
      splitterBg: "#24283B", punctuation: "#A9B1D6",
      keyword: "#F7768E", function: "#7AA2F7", string: "#9ECE6A",
      number: "#FF9E64", comment: "#565F89", type: "#E0AF68",
      variable: "#C0CAF5", operator: "#89DDFF", constant: "#FF9E64",
      tag: "#F7768E", attribute: "#7DCFFF",
    },
    yaml: `name: "Tokyo Night"
editor:
  background: "#1A1B26"
  foreground: "#A9B1D6"
  cursor: "#C0CAF5"
  selection: "#33467C"
syntax:
  keyword: "#F7768E"
  function: "#7AA2F7"
  string: "#9ECE6A"
  number: "#FF9E64"
  comment: "#565F89"`,
  },
  {
    id: "gruvbox-dark",
    name: "Gruvbox Dark",
    author: "Token",
    description: "Retro groove color scheme with warm, earthy tones",
    fileName: "gruvbox-dark.yaml",
    isLight: false,
    colors: {
      bg: "#282828", fg: "#EBDBB2", currentLine: "#32302F", cursor: "#EBDBB2",
      selection: "#504945", gutterBg: "#282828", gutterFg: "#665C54",
      sidebarBg: "#1D2021", statusBg: "#504945", statusFg: "#EBDBB2",
      gutterFgActive: "#EBDBB2", gutterBorder: "#3C3836",
      tabBarBg: "#1D2021", tabBarActiveBg: "#282828", tabBarActiveFg: "#EBDBB2",
      tabBarInactiveBg: "#1D2021", tabBarInactiveFg: "#928374", tabBarBorder: "#3C3836",
      splitterBg: "#3C3836", punctuation: "#EBDBB2",
      keyword: "#FB4934", function: "#FABD2F", string: "#B8BB26",
      number: "#D3869B", comment: "#928374", type: "#83A598",
      variable: "#EBDBB2", operator: "#EBDBB2", constant: "#D3869B",
      tag: "#FB4934", attribute: "#B8BB26",
    },
    yaml: `name: "Gruvbox Dark"
editor:
  background: "#282828"
  foreground: "#EBDBB2"
  cursor: "#EBDBB2"
  selection: "#504945"
syntax:
  keyword: "#FB4934"
  function: "#FABD2F"
  string: "#B8BB26"
  number: "#D3869B"
  comment: "#928374"`,
  },
];
