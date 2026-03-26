import { SearchGlass } from "../../../assets/icons";
import s from "./search.module.css";

type Props = {
  value: string;
  onChange: (value: string) => void;
};

export default function Search({ value, onChange }: Props) {
  return (
    <div className={s.search}>
      <label htmlFor="search" className={s.icon}>
        <SearchGlass />
      </label>
      <input
        className={s.input}
        id="search"
        type="text"
        placeholder="검색"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
