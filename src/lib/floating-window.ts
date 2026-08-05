export const FLOATING_MINI_MAX_HEIGHT = 120;

export function isMiniFloatingHeight(height: number) {
  return height <= FLOATING_MINI_MAX_HEIGHT;
}
