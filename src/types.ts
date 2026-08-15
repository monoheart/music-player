export type PlayMode = "order" | "shuffle" | "repeat";

export type SortMode = "custom" | "title" | "artist" | "album";

export type SortDir = "asc" | "desc";

export const SORT_OPTIONS: { value: SortMode; label: string }[] = [
  { value: "custom", label: "自定义" },
  { value: "title", label: "标题" },
  { value: "artist", label: "歌手" },
  { value: "album", label: "专辑" },
];

export interface Song {
  path: string;
  title: string;
  artist: string;
  album: string;
  duration: number;
  format: string;
  track: number | null;
  year: number | null;
  genre: string;
  fav: number;
  sort_pos: number;
}

export interface LastState {
  song_path: string | null;
  position: number;
  queue: string[];
  volume: number;
  play_mode: PlayMode;
  sort_mode: SortMode;
  sort_dir: SortDir;
  custom_sort: boolean;
}

export interface Playlist {
  id: number;
  name: string;
  count: number;
}
