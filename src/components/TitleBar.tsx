import "./TitleBar.css";

const TitleBar = ({ appName, version }: { appName: string; version: string }) => {
  return (
    <div className="title-bar">
      <div className="traffic-lights">
        <div className="circle red" />
        <div className="circle yellow" />
        <div className="circle green" />
      </div>
      <div className="title">
        {appName} <span className="version">v{version}</span>
      </div>
    </div>
  );
};

export default TitleBar;