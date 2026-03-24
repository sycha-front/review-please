import { H1 } from "../../../common/typo";
import s from "./header.module.css";

type HeaderProps = {
  // reviewSummary: string;
};

export default function Header({}: HeaderProps) {
  return (
    <header className={s.header}>
      <H1>Review-please</H1>
      {/* <P3>{reviewSummary}</P3> */}
    </header>
  );
}
