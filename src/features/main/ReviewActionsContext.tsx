import {
  createContext,
  useContext,
  type PropsWithChildren,
} from "react";
import type { UseReviewDumpResult } from "../../hooks/useReviewDump";

type ReviewActionsContextValue = Pick<
  UseReviewDumpResult,
  "updateDeadline" | "updateStatus"
>;

const ReviewActionsContext = createContext<ReviewActionsContextValue | null>(
  null,
);

type ReviewActionsProviderProps = PropsWithChildren<ReviewActionsContextValue>;

export function ReviewActionsProvider({
  children,
  updateDeadline,
  updateStatus,
}: ReviewActionsProviderProps) {
  return (
    <ReviewActionsContext.Provider value={{ updateDeadline, updateStatus }}>
      {children}
    </ReviewActionsContext.Provider>
  );
}

export function useReviewActions() {
  const value = useContext(ReviewActionsContext);

  if (!value) {
    throw new Error("useReviewActions must be used within ReviewActionsProvider");
  }

  return value;
}
