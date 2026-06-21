export const chartColors = [
  '#78a8d8',
  '#f2a6a6',
  '#91cfa6',
  '#c9a7e8',
  '#f2c879',
  '#86d2cf',
  '#e7a4cf',
  '#b6c47d',
  '#d7a57c',
  '#9ea8e8'
];

export function chartColorForAssetIndex(index: number) {
  return chartColors[index % chartColors.length];
}

