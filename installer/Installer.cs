using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

internal static class Installer
{
    private const string AppName = "Matto Chal\u00E9s";
    private const string AppVersion = "0.1.73";
    private static readonly string LocalAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
    private static readonly string InstallDir = Path.Combine(LocalAppData, "Programs", "Matto Chales");
    private static readonly string ConfigDir = Path.Combine(LocalAppData, "Matto Chales", "Matto Chales", "config");

    [STAThread]
    private static void Main(string[] args)
    {
        try
        {
            if (args.Length > 0 && args[0] == "--apply-update")
            {
                ApplyUpdate(args);
                return;
            }
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new SetupWindow());
        }
        catch (Exception error)
        {
            string log = Path.Combine(Path.GetTempPath(), "matto-chales-installer-error.txt");
            try { File.WriteAllText(log, error.ToString()); } catch { }
            MessageBox.Show("O instalador encontrou um erro ao iniciar.\n\n" + error.Message +
                "\n\nDetalhes: " + log, AppName, MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private static void ApplyUpdate(string[] args)
    {
        int processId;
        if (args.Length < 2 || !Int32.TryParse(args[1], out processId))
            throw new ArgumentException("Parâmetros de atualização inválidos.");
        try { Process.GetProcessById(processId).WaitForExit(30000); } catch (ArgumentException) { }
        Directory.CreateDirectory(InstallDir);
        string appPath = Path.Combine(InstallDir, "MattoChales.exe");
        string tempPath = appPath + ".update";
        string backupPath = appPath + ".previous";
        using (Stream input = Assembly.GetExecutingAssembly().GetManifestResourceStream("MattoChales.exe"))
        {
            if (input == null) throw new InvalidOperationException("O executável atualizado não está no pacote.");
            using (var output = new FileStream(tempPath, FileMode.Create, FileAccess.Write, FileShare.None)) input.CopyTo(output);
        }
        try
        {
            if (File.Exists(backupPath)) File.Delete(backupPath);
            if (File.Exists(appPath)) File.Move(appPath, backupPath);
            File.Move(tempPath, appPath);
        }
        catch
        {
            if (!File.Exists(appPath) && File.Exists(backupPath)) File.Move(backupPath, appPath);
            throw;
        }
        try { Process.Start(appPath); }
        catch
        {
            if (File.Exists(backupPath)) { File.Delete(appPath); File.Move(backupPath, appPath); }
            throw;
        }
    }

    private sealed class SetupWindow : Form
    {
        private readonly Panel content = new Panel { Dock = DockStyle.Fill };
        private readonly RoundedButton backButton = new RoundedButton { Outline = true };
        private readonly RoundedButton nextButton = new RoundedButton();
        private readonly Label stepsLabel = new Label();
        private readonly TextBox profileName = new TextBox();
        private readonly AvatarPreview avatar = new AvatarPreview();
        private readonly Label profileError = new Label();
        private readonly StyledProgressBar installProgress = new StyledProgressBar();
        private readonly Image forestImage;
        private readonly Image cabinImage;
        private ForestPanel welcomePanel;
        private Panel profilePanel;
        private ForestPanel installPanel;
        private ForestPanel completePanel;
        private string selectedPhotoPath;
        private int currentStage = 1;
        private bool installationComplete;

        public SetupWindow()
        {
            Text = AppName + " — Instalação";
            FormBorderStyle = FormBorderStyle.None;
            WindowState = FormWindowState.Maximized;
            StartPosition = FormStartPosition.Manual;
            Bounds = Screen.PrimaryScreen.Bounds;
            KeyPreview = true;
            BackColor = Color.FromArgb(25, 16, 11);
            ForeColor = Color.White;
            Font = new Font("Segoe UI", 10F);
            KeyDown += (sender, e) => { if (e.KeyCode == Keys.Escape && !installationComplete) Close(); };

            forestImage = ReadImageResource("InstallerWallpaper.jpg");
            cabinImage = ReadImageResource("ChaletMark.png");

            var shell = new TableLayoutPanel
            {
                Dock = DockStyle.Fill,
                ColumnCount = 1,
                RowCount = 3,
                BackColor = Color.FromArgb(25, 16, 11)
            };
            shell.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            shell.RowStyles.Add(new RowStyle(SizeType.Absolute, 60));
            shell.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            shell.RowStyles.Add(new RowStyle(SizeType.Absolute, 88));
            Controls.Add(shell);

            var topBar = CreateTopBar();
            var footer = CreateFooter();
            shell.Controls.Add(topBar, 0, 0);
            shell.Controls.Add(content, 0, 1);
            shell.Controls.Add(footer, 0, 2);
            BuildWelcomePanel();
            BuildProfilePanel();
            BuildInstallPanel();
            BuildCompletePanel();
            SetTransparentLabelBackgrounds(this);
            ShowStage(1);
        }

        private static void SetTransparentLabelBackgrounds(Control parent)
        {
            foreach (Control child in parent.Controls)
            {
                if (child is Label) child.BackColor = Color.Transparent;
                if (child.HasChildren) SetTransparentLabelBackgrounds(child);
            }
        }

        private Control CreateTopBar()
        {
            var bar = new Panel { Dock = DockStyle.Fill, BackColor = Color.FromArgb(31, 19, 12) };
            var mark = new PictureBox
            {
                Image = cabinImage,
                SizeMode = PictureBoxSizeMode.Zoom,
                Location = new Point(28, 10),
                Size = new Size(45, 40),
                BackColor = Color.FromArgb(31, 19, 12)
            };
            bar.Controls.Add(mark);
            bar.Controls.Add(new Label
            {
                Text = AppName,
                AutoSize = true,
                Location = new Point(84, 18),
                ForeColor = Color.FromArgb(246, 231, 207),
                Font = new Font("Segoe UI Semibold", 13F)
            });
            var close = new Button
            {
                Text = "×", Dock = DockStyle.Right, Width = 58,
                FlatStyle = FlatStyle.Flat, BackColor = Color.FromArgb(31, 19, 12),
                ForeColor = Color.FromArgb(246, 231, 207), Font = new Font("Segoe UI", 20F),
                Cursor = Cursors.Hand
            };
            close.FlatAppearance.BorderSize = 0;
            close.FlatAppearance.MouseOverBackColor = Color.FromArgb(105, 51, 27);
            close.Click += (sender, e) => Close();
            bar.Controls.Add(close);
            return bar;
        }

        private Control CreateFooter()
        {
            var footer = new Panel { Dock = DockStyle.Fill, BackColor = Color.FromArgb(31, 19, 12) };
            stepsLabel.Text = "ETAPA 1 DE 3";
            stepsLabel.AutoSize = true;
            stepsLabel.Location = new Point(32, 34);
            stepsLabel.ForeColor = Color.FromArgb(190, 160, 126);
            stepsLabel.Font = new Font("Segoe UI", 9F, FontStyle.Bold);
            footer.Controls.Add(stepsLabel);

            backButton.Text = "Voltar";
            backButton.Size = new Size(112, 46);
            backButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            backButton.BorderColor = Color.FromArgb(113, 73, 42);
            backButton.ForeColor = Color.FromArgb(246, 231, 207);
            backButton.BackColor = Color.FromArgb(39, 25, 17);
            backButton.Click += (sender, e) => ShowStage(currentStage - 1);
            footer.Controls.Add(backButton);

            nextButton.Text = "Avançar";
            nextButton.Size = new Size(260, 46);
            nextButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            nextButton.ForeColor = Color.White;
            nextButton.BackColor = Color.FromArgb(157, 89, 39);
            nextButton.Font = new Font("Segoe UI Semibold", 10F);
            nextButton.Click += Next;
            footer.Controls.Add(nextButton);
            footer.Resize += (sender, e) =>
            {
                LayoutFooterButtons(footer);
            };
            LayoutFooterButtons(footer);
            return footer;
        }

        private void LayoutFooterButtons(Panel footer)
        {
            nextButton.Left = Math.Max(footer.Width - nextButton.Width - 32, 0);
            backButton.Left = Math.Max(nextButton.Left - backButton.Width - 16, 0);
            backButton.Top = 21;
            nextButton.Top = 21;
        }

        private void BuildWelcomePanel()
        {
            welcomePanel = new ForestPanel(forestImage) { Dock = DockStyle.Fill, TextMode = ForestTextMode.Welcome };
        }

        private void BuildProfilePanel()
        {
            profilePanel = new Panel { Dock = DockStyle.Fill, BackColor = Color.FromArgb(238, 238, 229) };
            var columns = new TableLayoutPanel
            {
                Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 1,
                BackColor = Color.FromArgb(238, 238, 229)
            };
            columns.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 40));
            columns.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 60));
            columns.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            profilePanel.Controls.Add(columns);

            var photoSide = new ForestPanel(forestImage) { Dock = DockStyle.Fill, TextMode = ForestTextMode.Profile };
            columns.Controls.Add(photoSide, 0, 0);

            var formSide = new Panel { Dock = DockStyle.Fill, BackColor = Color.FromArgb(238, 238, 229) };
            columns.Controls.Add(formSide, 1, 0);
            var heading = new Label
            {
                Text = "Configure seu perfil", AutoSize = true,
                Font = new Font("Segoe UI Semibold", 25F, FontStyle.Bold), ForeColor = Color.FromArgb(27, 48, 33)
            };
            var subheading = new Label
            {
                Text = "Como podemos chamar você no aplicativo?", AutoSize = true,
                Font = new Font("Segoe UI", 11F), ForeColor = Color.FromArgb(91, 103, 91)
            };
            avatar.Size = new Size(126, 126);
            avatar.BackColor = Color.FromArgb(238, 238, 229);
            var photoButton = new Button
            {
                Text = "Adicionar foto de perfil", Size = new Size(204, 42),
                FlatStyle = FlatStyle.Flat, BackColor = Color.FromArgb(225, 230, 216),
                ForeColor = Color.FromArgb(33, 61, 38), Font = new Font("Segoe UI Semibold", 9.5F), Cursor = Cursors.Hand
            };
            photoButton.FlatAppearance.BorderColor = Color.FromArgb(192, 202, 185);
            photoButton.Click += ChoosePhoto;
            var nameLabel = new Label
            {
                Text = "Seu nome", AutoSize = true, Font = new Font("Segoe UI Semibold", 10F),
                ForeColor = Color.FromArgb(43, 52, 43)
            };
            profileName.BorderStyle = BorderStyle.FixedSingle;
            profileName.Font = new Font("Segoe UI", 13F);
            profileName.MaxLength = 60;
            var hint = new Label
            {
                Text = "Sua foto e seu nome aparecem no perfil do aplicativo.\nA foto é opcional e pode ser alterada depois.",
                AutoSize = true, ForeColor = Color.FromArgb(104, 112, 102), Font = new Font("Segoe UI", 9.5F)
            };
            profileError.Text = "";
            profileError.AutoSize = true;
            profileError.ForeColor = Color.FromArgb(150, 64, 52);

            formSide.Controls.Add(heading);
            formSide.Controls.Add(subheading);
            formSide.Controls.Add(avatar);
            formSide.Controls.Add(photoButton);
            formSide.Controls.Add(nameLabel);
            formSide.Controls.Add(profileName);
            formSide.Controls.Add(hint);
            formSide.Controls.Add(profileError);
            formSide.Resize += (sender, e) =>
            {
                int center = formSide.Width / 2;
                int h = formSide.Height;
                heading.Location = new Point(center - Math.Min(heading.Width, 520) / 2, Math.Max(48, h / 2 - 270));
                subheading.Location = new Point(center - Math.Min(subheading.Width, 520) / 2, heading.Bottom + 8);
                avatar.Location = new Point(center - avatar.Width / 2, subheading.Bottom + 27);
                photoButton.Location = new Point(center - photoButton.Width / 2, avatar.Bottom + 12);
                int fieldWidth = Math.Min(520, formSide.Width - 100);
                nameLabel.Location = new Point(center - fieldWidth / 2, photoButton.Bottom + 29);
                profileName.SetBounds(center - fieldWidth / 2, nameLabel.Bottom + 8, fieldWidth, 46);
                hint.Location = new Point(center - fieldWidth / 2, profileName.Bottom + 12);
                profileError.Location = new Point(center - fieldWidth / 2, hint.Bottom + 6);
            };
        }

        private void BuildInstallPanel()
        {
            installPanel = new ForestPanel(forestImage) { Dock = DockStyle.Fill, TextMode = ForestTextMode.Installing };
            installPanel.SetOverlayStatus("Aguardando início da instalação");
            installProgress.Size = new Size(600, 24);
            installPanel.Controls.Add(installProgress);
            installPanel.Resize += (sender, e) =>
            {
                int centerX = installPanel.Width / 2;
                int centerY = installPanel.Height / 2;
                installProgress.Location = new Point(centerX - installProgress.Width / 2, centerY + 84);
            };
        }

        private void BuildCompletePanel()
        {
            completePanel = new ForestPanel(forestImage) { Dock = DockStyle.Fill, TextMode = ForestTextMode.Complete };
        }

        private void ShowStage(int stage)
        {
            currentStage = stage;
            content.Controls.Clear();
            backButton.Visible = stage == 2 || stage == 3;
            backButton.Enabled = stage == 2;
            nextButton.Visible = true;
            nextButton.Enabled = true;
            if (stage == 1)
            {
                content.Controls.Add(welcomePanel);
                stepsLabel.Text = "ETAPA 1 DE 3  ·  BOAS-VINDAS";
                nextButton.Text = "Avançar";
            }
            else if (stage == 2)
            {
                content.Controls.Add(profilePanel);
                stepsLabel.Text = "ETAPA 2 DE 3  ·  SEU PERFIL";
                nextButton.Text = "Finalizar configuração e começar a instalar";
                nextButton.Width = 360;
            }
            else if (stage == 3)
            {
                content.Controls.Add(installPanel);
                stepsLabel.Text = "ETAPA 3 DE 3  ·  INSTALAÇÃO";
                nextButton.Text = "Instalando...";
                nextButton.Enabled = false;
                nextButton.Width = 260;
            }
            else
            {
                content.Controls.Add(completePanel);
                stepsLabel.Text = "CONFIGURAÇÃO CONCLUÍDA";
                backButton.Visible = false;
                nextButton.Text = "Abrir Matto Chalés";
                nextButton.Enabled = true;
                nextButton.Width = 230;
            }
            LayoutFooterButtons((Panel)nextButton.Parent);
            if (stage == 3) StartInstall();
        }

        private void Next(object sender, EventArgs e)
        {
            if (currentStage == 1)
            {
                ShowStage(2);
                return;
            }
            if (currentStage == 2)
            {
                string name = profileName.Text.Trim();
                if (name.Length < 2)
                {
                    profileError.Text = "Digite um nome com pelo menos 2 caracteres.";
                    profileName.Focus();
                    return;
                }
                profileError.Text = "";
                ShowStage(3);
                return;
            }
            if (currentStage == 4)
            {
                try { System.Diagnostics.Process.Start(Path.Combine(InstallDir, "MattoChales.exe")); }
                catch (Exception ex) { MessageBox.Show(this, "Não foi possível abrir o aplicativo:\n" + ex.Message, AppName, MessageBoxButtons.OK, MessageBoxIcon.Error); }
                Close();
            }
        }

        private void ChoosePhoto(object sender, EventArgs e)
        {
            using (var dialog = new OpenFileDialog())
            {
                dialog.Title = "Escolha sua foto de perfil";
                dialog.Filter = "Fotos de perfil|*.png;*.jpg;*.jpeg;*.gif|Todos os arquivos|*.*";
                dialog.Multiselect = false;
                if (dialog.ShowDialog(this) != DialogResult.OK) return;
                try
                {
                    using (var image = Image.FromFile(dialog.FileName))
                        avatar.ProfileImage = new Bitmap(image);
                    selectedPhotoPath = dialog.FileName;
                }
                catch (Exception ex)
                {
                    MessageBox.Show(this, "Não foi possível abrir essa imagem:\n" + ex.Message, AppName, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                }
            }
        }

        private void StartInstall()
        {
            installProgress.Value = 0;
            installPanel.SetOverlayStatus("Preparando os arquivos...");
            var worker = new BackgroundWorker { WorkerReportsProgress = true };
            worker.ProgressChanged += (sender, e) =>
            {
                var report = (InstallReport)e.UserState;
                installProgress.Value = Math.Max(0, Math.Min(100, report.Percent));
                installPanel.SetOverlayStatus(report.Message);
            };
            worker.DoWork += (sender, e) => InstallFiles(worker);
            worker.RunWorkerCompleted += (sender, e) =>
            {
                if (e.Error != null)
                {
                    MessageBox.Show(this, "A instalação não foi concluída:\n" + e.Error.Message, AppName, MessageBoxButtons.OK, MessageBoxIcon.Error);
                    ShowStage(2);
                    return;
                }
                installationComplete = true;
                ShowStage(4);
            };
            worker.RunWorkerAsync();
        }

        private void InstallFiles(BackgroundWorker worker)
        {
            var elapsed = Stopwatch.StartNew();
            Directory.CreateDirectory(InstallDir);
            Directory.CreateDirectory(ConfigDir);
            string appExe = Path.Combine(InstallDir, "MattoChales.exe");
            worker.ReportProgress(2, new InstallReport(2, "Preparando a pasta de instalação..."));

            using (Stream input = Assembly.GetExecutingAssembly().GetManifestResourceStream("MattoChales.exe"))
            {
                if (input == null) throw new InvalidOperationException("O executável não está incluído no instalador.");
                using (var output = new FileStream(appExe, FileMode.Create, FileAccess.Write, FileShare.None))
                {
                    byte[] buffer = new byte[64 * 1024];
                    long copied = 0;
                    int count;
                    while ((count = input.Read(buffer, 0, buffer.Length)) > 0)
                    {
                        output.Write(buffer, 0, count);
                        copied += count;
                        int percent = 5 + (int)(copied * 78 / input.Length);
                        worker.ReportProgress(percent, new InstallReport(percent, "Instalando o aplicativo..."));
                    }
                }
            }

            worker.ReportProgress(85, new InstallReport(85, "Salvando seu perfil..."));
            string savedPhoto = null;
            if (!String.IsNullOrEmpty(selectedPhotoPath))
            {
                string extension = Path.GetExtension(selectedPhotoPath).ToLowerInvariant();
                if (extension != ".png" && extension != ".jpg" && extension != ".jpeg" && extension != ".gif")
                    throw new InvalidOperationException("Use uma imagem PNG, JPG ou GIF para a foto de perfil.");
                savedPhoto = Path.Combine(ConfigDir, "foto-perfil" + extension);
                File.Copy(selectedPhotoPath, savedPhoto, true);
            }
            string profileJson = "{\"name\":\"" + EscapeJson(profileName.Text.Trim()) + "\",\"photoPath\":\"" + EscapeJson(savedPhoto ?? "") + "\"}";
            File.WriteAllText(Path.Combine(ConfigDir, "perfil.json"), profileJson, new UTF8Encoding(false));

            worker.ReportProgress(92, new InstallReport(92, "Criando atalhos e registro de desinstalação..."));
            string iconFile = Path.Combine(InstallDir, "MattoChales.ico");
            using (Stream iconInput = Assembly.GetExecutingAssembly().GetManifestResourceStream("MattoChales.ico"))
            using (var iconOutput = new FileStream(iconFile, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                if (iconInput == null) throw new InvalidOperationException("O ícone do aplicativo não está incluído no instalador.");
                iconInput.CopyTo(iconOutput);
            }
            CreateShortcut(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), "Matto Chal\u00E9s", "Matto Chal\u00E9s.lnk"), appExe, iconFile);
            CreateShortcut(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory), "Matto Chal\u00E9s.lnk"), appExe, iconFile);
            RegisterUninstaller(appExe, Path.Combine(InstallDir, "uninstall.ps1"));

            var minimumInstallTime = TimeSpan.FromSeconds(5);
            while (elapsed.Elapsed < minimumInstallTime)
            {
                double ratio = elapsed.Elapsed.TotalMilliseconds / minimumInstallTime.TotalMilliseconds;
                int percent = 92 + (int)(ratio * 8.0);
                worker.ReportProgress(percent, new InstallReport(percent, "Finalizando a instalação..."));
                Thread.Sleep(100);
            }
            worker.ReportProgress(100, new InstallReport(100, "Instalação concluída."));
        }

        private void RegisterUninstaller(string appExe, string uninstaller)
        {
            string script = @"
$ErrorActionPreference = 'SilentlyContinue'
$installDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$startMenuShortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Matto Chalés\Matto Chalés.lnk'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Matto Chalés.lnk'
Remove-Item -LiteralPath $startMenuShortcut -Force
Remove-Item -LiteralPath (Split-Path -Parent $startMenuShortcut) -Recurse -Force
Remove-Item -LiteralPath $desktopShortcut -Force
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MattoChales' -Recurse -Force
Start-Process -FilePath $env:ComSpec -ArgumentList @('/c', ('timeout /t 2 /nobreak >nul & rmdir /s /q ' + [char]34 + $installDir + [char]34)) -WindowStyle Hidden
";
            File.WriteAllText(uninstaller, script, new UTF8Encoding(true));
            using (Microsoft.Win32.RegistryKey key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\MattoChales"))
            {
                key.SetValue("DisplayName", AppName);
                key.SetValue("DisplayVersion", AppVersion);
                key.SetValue("Publisher", AppName);
                key.SetValue("InstallLocation", InstallDir);
                key.SetValue("DisplayIcon", Path.Combine(InstallDir, "MattoChales.ico"));
                key.SetValue("UninstallString", "powershell.exe -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File \"" + uninstaller + "\"");
                key.SetValue("NoModify", 1, Microsoft.Win32.RegistryValueKind.DWord);
                key.SetValue("NoRepair", 1, Microsoft.Win32.RegistryValueKind.DWord);
            }
        }

        private static void CreateShortcut(string path, string target, string iconFile)
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            Type shellType = Type.GetTypeFromProgID("WScript.Shell");
            dynamic shell = Activator.CreateInstance(shellType);
            dynamic shortcut = shell.CreateShortcut(path);
            shortcut.TargetPath = target;
            shortcut.WorkingDirectory = InstallDir;
            shortcut.IconLocation = iconFile;
            shortcut.Description = "Aplicativo Matto Chal\u00E9s";
            shortcut.Save();
            Marshal.FinalReleaseComObject(shortcut);
            Marshal.FinalReleaseComObject(shell);
        }

        private static string EscapeJson(string value)
        {
            return value.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\r", "\\r").Replace("\n", "\\n");
        }

        private static Image ReadImageResource(string name)
        {
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(name))
            {
                if (stream == null) throw new InvalidOperationException("Imagem do instalador ausente: " + name);
                using (var source = Image.FromStream(stream)) return new Bitmap(source);
            }
        }
    }

    private sealed class InstallReport
    {
        public readonly int Percent;
        public readonly string Message;
        public InstallReport(int percent, string message) { Percent = percent; Message = message; }
    }

    private sealed class RoundedButton : Control
    {
        public bool Outline { get; set; }
        public Color BorderColor { get; set; }
        private bool pressed;

        public RoundedButton()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint |
                ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw | ControlStyles.Selectable, true);
            TabStop = true;
            Cursor = Cursors.Hand;
            BorderColor = Color.Transparent;
        }

        protected override void OnPaintBackground(PaintEventArgs e)
        {
            Color background = Parent == null ? SystemColors.Control : Parent.BackColor;
            using (var brush = new SolidBrush(background)) e.Graphics.FillRectangle(brush, ClientRectangle);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            var rect = new RectangleF(1F, 1F, Math.Max(1, Width - 2), Math.Max(1, Height - 2));
            float radius = Math.Min(14F, Math.Min(rect.Width, rect.Height) / 2F);
            using (var path = RoundedPath(rect, radius))
            {
                Color fill = BackColor;
                if (!Enabled) fill = Adjust(fill, -20);
                else if (pressed) fill = Adjust(fill, -18);
                else if (ClientRectangle.Contains(PointToClient(Cursor.Position))) fill = Adjust(fill, 12);
                using (var brush = new SolidBrush(fill)) e.Graphics.FillPath(brush, path);
                if (Outline)
                {
                    using (var pen = new Pen(BorderColor, 1F)) e.Graphics.DrawPath(pen, path);
                }
            }
            var textColor = Enabled ? ForeColor : Color.FromArgb(145, 145, 138);
            TextRenderer.DrawText(e.Graphics, Text, Font, ClientRectangle, textColor,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter |
                TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
            base.OnPaint(e);
        }

        protected override void OnMouseEnter(EventArgs e) { base.OnMouseEnter(e); Invalidate(); }
        protected override void OnMouseLeave(EventArgs e) { base.OnMouseLeave(e); Invalidate(); }
        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (e.Button == MouseButtons.Left && Enabled) { pressed = true; Focus(); }
            Invalidate();
        }
        protected override void OnMouseUp(MouseEventArgs e)
        {
            pressed = false;
            base.OnMouseUp(e);
            Invalidate();
        }
        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (Enabled && (e.KeyCode == Keys.Enter || e.KeyCode == Keys.Space))
            {
                OnClick(EventArgs.Empty);
                e.Handled = true;
            }
            base.OnKeyDown(e);
        }

        private static Color Adjust(Color color, int amount)
        {
            return Color.FromArgb(color.A,
                Math.Max(0, Math.Min(255, color.R + amount)),
                Math.Max(0, Math.Min(255, color.G + amount)),
                Math.Max(0, Math.Min(255, color.B + amount)));
        }

        private static GraphicsPath RoundedPath(RectangleF bounds, float radius)
        {
            float diameter = radius * 2F;
            var path = new GraphicsPath();
            path.AddArc(bounds.X, bounds.Y, diameter, diameter, 180, 90);
            path.AddArc(bounds.Right - diameter, bounds.Y, diameter, diameter, 270, 90);
            path.AddArc(bounds.Right - diameter, bounds.Bottom - diameter, diameter, diameter, 0, 90);
            path.AddArc(bounds.X, bounds.Bottom - diameter, diameter, diameter, 90, 90);
            path.CloseFigure();
            return path;
        }
    }

    private sealed class StyledProgressBar : Control
    {
        private int value;
        public int Value
        {
            get { return value; }
            set { this.value = Math.Max(0, Math.Min(100, value)); Invalidate(); }
        }

        public StyledProgressBar()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint |
                ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw |
                ControlStyles.SupportsTransparentBackColor, true);
            BackColor = Color.Transparent;
        }

        protected override void OnPaintBackground(PaintEventArgs e)
        {
            if (Parent != null)
            {
                var state = e.Graphics.Save();
                e.Graphics.TranslateTransform(-Left, -Top);
                var args = new PaintEventArgs(e.Graphics, new Rectangle(Left, Top, Width, Height));
                InvokePaintBackground(Parent, args);
                InvokePaint(Parent, args);
                e.Graphics.Restore(state);
            }
            else base.OnPaintBackground(e);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            int barWidth = Math.Max(1, Width - 58);
            var track = new Rectangle(1, 5, barWidth - 2, 14);
            using (var trackPath = ProgressPath(track, 7F))
            {
                using (var trackBrush = new LinearGradientBrush(track,
                Color.FromArgb(48, 29, 18), Color.FromArgb(25, 16, 11), 90F))
                    e.Graphics.FillPath(trackBrush, trackPath);
                using (var border = new Pen(Color.FromArgb(186, 143, 91), 1F))
                    e.Graphics.DrawPath(border, trackPath);

                int innerWidth = Math.Max(0, (track.Width - 4) * Value / 100);
                if (innerWidth > 0)
                {
                    var fill = new Rectangle(track.X + 2, track.Y + 2, innerWidth, track.Height - 4);
                    using (var fillPath = ProgressPath(fill, Math.Min(5F, fill.Height / 2F)))
                    {
                        using (var fillBrush = new LinearGradientBrush(fill,
                            Color.FromArgb(222, 157, 87), Color.FromArgb(148, 76, 32), 90F))
                            e.Graphics.FillPath(fillBrush, fillPath);
                        if (fill.Width > 12)
                        {
                            using (var shine = new Pen(Color.FromArgb(68, 255, 255, 239), 1F))
                                e.Graphics.DrawLine(shine, fill.X + 5, fill.Y + 2, fill.Right - 5, fill.Y + 2);
                        }
                    }
                }
            }

            var percentBounds = new Rectangle(barWidth + 7, 2, 49, 21);
            using (var percentFont = new Font("Segoe UI Semibold", 9F, FontStyle.Bold))
                TextRenderer.DrawText(e.Graphics, Value.ToString() + "%", percentFont,
                    percentBounds, Color.FromArgb(244, 239, 224),
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
            base.OnPaint(e);
        }

        private static GraphicsPath ProgressPath(Rectangle bounds, float radius)
        {
            float diameter = radius * 2F;
            var path = new GraphicsPath();
            path.AddArc(bounds.X, bounds.Y, diameter, diameter, 180, 90);
            path.AddArc(bounds.Right - diameter, bounds.Y, diameter, diameter, 270, 90);
            path.AddArc(bounds.Right - diameter, bounds.Bottom - diameter, diameter, diameter, 0, 90);
            path.AddArc(bounds.X, bounds.Bottom - diameter, diameter, diameter, 90, 90);
            path.CloseFigure();
            return path;
        }
    }

    private enum ForestTextMode { None, Welcome, Profile, Installing, Complete }

    private sealed class ForestPanel : Panel
    {
        private readonly Image image;
        public ForestTextMode TextMode { get; set; }
        public string OverlayStatus { get; private set; }
        public ForestPanel(Image image) { this.image = CreateSoftBackground(image); DoubleBuffered = true; SetStyle(ControlStyles.ResizeRedraw, true); }

        public void SetOverlayStatus(string value)
        {
            OverlayStatus = value;
            Invalidate();
        }

        private static Image CreateSoftBackground(Image source)
        {
            int width = Math.Max(1, source.Width * 2 / 3);
            int height = Math.Max(1, source.Height * 2 / 3);
            var softened = new Bitmap(width, height);
            using (Graphics graphics = Graphics.FromImage(softened))
            {
                graphics.InterpolationMode = InterpolationMode.HighQualityBilinear;
                graphics.DrawImage(source, new Rectangle(0, 0, width, height));
            }
            return softened;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            if (image == null || Width == 0 || Height == 0) return;
            float targetRatio = (float)Width / Height;
            float imageRatio = (float)image.Width / image.Height;
            Rectangle source;
            if (imageRatio > targetRatio)
            {
                int width = (int)(image.Height * targetRatio);
                source = new Rectangle((image.Width - width) / 2, 0, width, image.Height);
            }
            else
            {
                int height = (int)(image.Width / targetRatio);
                source = new Rectangle(0, (image.Height - height) / 2, image.Width, height);
            }
            e.Graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            e.Graphics.DrawImage(image, ClientRectangle, source, GraphicsUnit.Pixel);
            using (var shade = new LinearGradientBrush(ClientRectangle,
                Color.FromArgb(190, 13, 19, 15), Color.FromArgb(95, 13, 19, 15), 0F))
                e.Graphics.FillRectangle(shade, ClientRectangle);

            DrawOverlayText(e.Graphics);
        }

        private void DrawOverlayText(Graphics graphics)
        {
            int centerY = Height / 2;
            int left = Math.Max(38, Width / 12);
            if (TextMode == ForestTextMode.Welcome)
            {
                DrawText(graphics, "Bem-vindo ao\nMatto Chalés", new Font("Segoe UI Semibold", 44F, FontStyle.Bold),
                    new Rectangle(left, centerY - 180, Math.Min(780, Width - left * 2), 180), Color.White,
                    TextFormatFlags.WordBreak | TextFormatFlags.NoPadding);
                DrawText(graphics, "Seu refúgio começa aqui.", new Font("Segoe UI", 20F),
                    new Rectangle(left + 6, centerY + 8, Math.Max(100, Width - left * 2), 36), Color.FromArgb(238, 233, 215),
                    TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
                DrawText(graphics, "Um espaço pensado para acolher, conectar com a natureza\ne deixar a rotina do seu chalé mais simples.", new Font("Segoe UI", 12F),
                    new Rectangle(left + 8, centerY + 58, Math.Min(690, Width - left * 2), 54), Color.FromArgb(227, 227, 216),
                    TextFormatFlags.WordBreak | TextFormatFlags.NoPadding);
            }
            else if (TextMode == ForestTextMode.Profile)
            {
                DrawText(graphics, "SEU ESPAÇO.\nSEU JEITO.", new Font("Segoe UI", 36F, FontStyle.Bold),
                    new Rectangle(left, centerY - 74, Math.Max(100, Width - left * 2), 148), Color.White,
                    TextFormatFlags.WordBreak | TextFormatFlags.NoPadding);
            }
            else if (TextMode == ForestTextMode.Installing)
            {
                int textWidth = Math.Min(820, Width - 60);
                DrawText(graphics, "Preparando seu espaço", new Font("Segoe UI Semibold", 30F, FontStyle.Bold),
                    new Rectangle((Width - textWidth) / 2, centerY - 104, textWidth, 46), Color.White,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
                DrawText(graphics, "Estamos instalando o Matto Chalés neste computador.", new Font("Segoe UI", 13F),
                    new Rectangle((Width - textWidth) / 2, centerY - 48, textWidth, 26), Color.FromArgb(225, 230, 221),
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
                DrawText(graphics, OverlayStatus ?? "Aguardando início da instalação", new Font("Segoe UI Semibold", 11F, FontStyle.Bold),
                    new Rectangle(Width / 2 - 270, centerY + 48, 540, 22), Color.White,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
            }
            else if (TextMode == ForestTextMode.Complete)
            {
                int textWidth = Math.Min(900, Width - 60);
                DrawText(graphics, "Tudo pronto para começar", new Font("Segoe UI Semibold", 31F, FontStyle.Bold),
                    new Rectangle((Width - textWidth) / 2, centerY - 52, textWidth, 48), Color.White,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
                DrawText(graphics, "O Matto Chalés foi instalado e seu perfil está configurado.", new Font("Segoe UI", 13F),
                    new Rectangle((Width - textWidth) / 2, centerY + 8, textWidth, 26), Color.FromArgb(228, 232, 222),
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
            }
        }

        private static void DrawText(Graphics graphics, string text, Font font, Rectangle bounds, Color color, TextFormatFlags flags)
        {
            using (font)
                TextRenderer.DrawText(graphics, text, font, bounds, color, flags);
        }
    }

    private sealed class AvatarPreview : Control
    {
        private Image profileImage;
        public Image ProfileImage
        {
            get { return profileImage; }
            set { profileImage = value; Invalidate(); }
        }
        public AvatarPreview() { DoubleBuffered = true; }
        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            var rect = new Rectangle(3, 3, Width - 6, Height - 6);
            using (var background = new SolidBrush(Color.FromArgb(236, 211, 178))) e.Graphics.FillEllipse(background, rect);
            if (profileImage != null)
            {
                using (var path = new GraphicsPath())
                {
                    path.AddEllipse(rect);
                    e.Graphics.SetClip(path);
                    float scale = Math.Max((float)rect.Width / profileImage.Width, (float)rect.Height / profileImage.Height);
                    int width = (int)(profileImage.Width * scale);
                    int height = (int)(profileImage.Height * scale);
                    e.Graphics.DrawImage(profileImage, new Rectangle(rect.X + (rect.Width - width) / 2, rect.Y + (rect.Height - height) / 2, width, height));
                    e.Graphics.ResetClip();
                }
            }
            else
            {
                using (var pen = new Pen(Color.FromArgb(133, 76, 38), 2.4F))
                using (var icon = new GraphicsPath())
                {
                    pen.StartCap = LineCap.Round;
                    pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    float scale = 2.4F;
                    float top = Height / 2F - 9F * scale;
                    float centerX = Width / 2F;
                    float headCenterY = top + 8F * scale;
                    float radius = 5F * scale;
                    icon.AddEllipse(centerX - radius, headCenterY - radius, radius * 2F, radius * 2F);

                    float shoulderRadius = 8F * scale;
                    float shoulderY = top + 21F * scale;
                    float tangent = shoulderRadius * 0.55228475F;
                    icon.StartFigure();
                    icon.AddBezier(
                        new PointF(centerX + shoulderRadius, shoulderY),
                        new PointF(centerX + shoulderRadius, shoulderY - tangent),
                        new PointF(centerX - shoulderRadius, shoulderY - tangent),
                        new PointF(centerX - shoulderRadius, shoulderY));
                    e.Graphics.DrawPath(pen, icon);
                }
            }
            using (var border = new Pen(Color.White, 4F)) e.Graphics.DrawEllipse(border, rect);
        }
    }
}
