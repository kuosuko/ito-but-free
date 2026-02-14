import "./TitleBar.css";

const TitleBar = ({ appName, version }: { appName: string; version: string }) => {
  return (
    <div className="title-bar">
      <span className="title-bar-name">{appName}</span>
      <span className="title-bar-version">v{version}</span>
    </div>
  );
};

export default TitleBar;
